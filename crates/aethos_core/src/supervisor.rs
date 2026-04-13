//! Phoenix-style supervisor for Tokio tasks.
//!
//! | Feature               | OTP                    | Aethos                            |
//! |-----------------------|------------------------|-----------------------------------|
//! | Restart policies      | permanent/transient/temporary | [`RestartPolicy`]            |
//! | Restart intensity     | max_restarts/max_seconds | [`RestartConfig::with_intensity`] |
//! | One-for-one           | `:one_for_one`         | [`SupervisorStrategy::OneForOne`] |
//! | One-for-all           | `:one_for_all`         | [`SupervisorStrategy::OneForAll`] |
//! | Dynamic children      | `DynamicSupervisor`    | [`DynamicSupervisor`]             |
//! | Graceful shutdown     | `Supervisor.stop/3`    | [`SupervisorHandle::shutdown`]    |

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;

// ── Strategy ──────────────────────────────────────────────────────────────────

/// How the supervisor reacts when a child crashes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SupervisorStrategy {
    /// Restart only the crashed child. Analogous to OTP `:one_for_one`.
    #[default]
    OneForOne,
    /// When any child crashes, stop all children and restart them all.
    /// Analogous to OTP `:one_for_all`.
    OneForAll,
}

// ── Restart policy ────────────────────────────────────────────────────────────

/// Defines when a supervised child should be restarted.
///
/// Mirrors Phoenix/OTP's `:permanent`, `:transient`, `:temporary` child types.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum RestartPolicy {
    /// Always restart, regardless of how the child exited.
    #[default]
    Permanent,
    /// Restart only on *abnormal* exit (panic or task cancellation).
    /// A child that returns `()` cleanly is not restarted.
    Transient,
    /// Never restart. The child runs once and is done.
    Temporary,
}

// ── Restart config ────────────────────────────────────────────────────────────

/// Restart intensity and timing configuration for a supervised child.
#[derive(Clone, Debug)]
pub struct RestartConfig {
    /// When to restart this child.
    pub policy: RestartPolicy,
    /// Maximum number of restarts within `window` before giving up.
    /// Analogous to Phoenix's `max_restarts`.
    pub max_restarts: u32,
    /// Sliding time window for counting restarts.
    /// Analogous to Phoenix's `max_seconds`.
    pub window: Duration,
    /// How long to wait before each restart attempt.
    pub backoff: Duration,
}

impl Default for RestartConfig {
    fn default() -> Self {
        Self {
            policy: RestartPolicy::Permanent,
            max_restarts: 3,
            window: Duration::from_secs(5),
            backoff: Duration::from_millis(500),
        }
    }
}

impl RestartConfig {
    /// Permanent child with default intensity (3 restarts / 5 s).
    pub fn permanent() -> Self {
        Self::default()
    }

    /// Transient child — restart only on panic / abnormal exit.
    pub fn transient() -> Self {
        Self { policy: RestartPolicy::Transient, ..Self::default() }
    }

    /// Temporary child — run once, never restart.
    pub fn temporary() -> Self {
        Self { policy: RestartPolicy::Temporary, ..Self::default() }
    }

    /// Override the exponential-backoff delay between restarts.
    pub fn with_backoff(mut self, backoff: Duration) -> Self {
        self.backoff = backoff;
        self
    }

    /// Override restart intensity: at most `max_restarts` within `window`.
    pub fn with_intensity(mut self, max_restarts: u32, window: Duration) -> Self {
        self.max_restarts = max_restarts;
        self.window = window;
        self
    }
}

// ── Child spec ────────────────────────────────────────────────────────────────

type Factory = Arc<dyn Fn() -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

/// Specification of a supervised child task.
///
/// `factory` is called each time the child needs to be (re)started.
pub struct ChildSpec {
    /// Human-readable identifier used in log messages.
    pub id: String,
    /// Restart behaviour for this child.
    pub config: RestartConfig,
    factory: Factory,
}

impl ChildSpec {
    /// Build a child spec.
    ///
    /// `f` is the factory — an `Fn` (not `FnOnce`) so it can be called on
    /// each restart.
    pub fn new(
        id: impl Into<String>,
        config: RestartConfig,
        f: impl Fn() -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync + 'static,
    ) -> Self {
        Self { id: id.into(), config, factory: Arc::new(f) }
    }
}

// ── SupervisorHandle ──────────────────────────────────────────────────────────

/// Returned by [`Supervisor::start`]. Provides graceful-shutdown control.
pub struct SupervisorHandle {
    join: JoinHandle<()>,
    shutdown_tx: watch::Sender<bool>,
}

impl SupervisorHandle {
    /// Signal all children to stop; wait up to `timeout`, then abandon.
    pub async fn shutdown(self, timeout: Duration) {
        let _ = self.shutdown_tx.send(true);
        match tokio::time::timeout(timeout, self.join).await {
            Ok(_) => {}
            Err(_) => tracing::warn!("supervisor shutdown timed out after {}ms", timeout.as_millis()),
        }
    }

    /// Block until all children have terminated (no shutdown signal sent).
    pub async fn wait(self) { let _ = self.join.await; }
}

// ── Static supervisor ─────────────────────────────────────────────────────────

/// Phoenix-style supervisor supporting `:one_for_one` and `:one_for_all` strategies.
#[derive(Default)]
pub struct Supervisor {
    specs: Vec<ChildSpec>,
    strategy: SupervisorStrategy,
}

impl Supervisor {
    pub fn one_for_one() -> Self { Self { strategy: SupervisorStrategy::OneForOne, ..Default::default() } }
    pub fn one_for_all() -> Self { Self { strategy: SupervisorStrategy::OneForAll, ..Default::default() } }

    /// Register a child with this supervisor.
    pub fn child(mut self, spec: ChildSpec) -> Self {
        self.specs.push(spec);
        self
    }

    /// Spawn all children and return a [`SupervisorHandle`] for shutdown control.
    pub fn start(self) -> SupervisorHandle {
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let join = match self.strategy {
            SupervisorStrategy::OneForOne => tokio::spawn(run_one_for_one(self.specs, shutdown_rx)),
            SupervisorStrategy::OneForAll => tokio::spawn(run_one_for_all(self.specs, shutdown_rx)),
        };
        SupervisorHandle { join, shutdown_tx }
    }
}

// ── DynamicSupervisor ─────────────────────────────────────────────────────────

enum DynCmd { Add(ChildSpec), Remove(String) }

/// Runtime-managed supervisor — children can be added/removed after start.
/// Each child is supervised with its own `:one_for_one` restart loop.
pub struct DynamicSupervisor {
    cmd_tx: mpsc::Sender<DynCmd>,
    shutdown_tx: watch::Sender<bool>,
    join: JoinHandle<()>,
}

impl DynamicSupervisor {
    pub fn start() -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel::<DynCmd>(64);
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let join = tokio::spawn(dynamic_loop(cmd_rx, shutdown_rx));
        Self { cmd_tx, shutdown_tx, join }
    }

    pub async fn add_child(&self, spec: ChildSpec) {
        let _ = self.cmd_tx.send(DynCmd::Add(spec)).await;
    }

    pub async fn terminate_child(&self, id: &str) {
        let _ = self.cmd_tx.send(DynCmd::Remove(id.to_owned())).await;
    }

    pub async fn shutdown(self, timeout: Duration) {
        let _ = self.shutdown_tx.send(true);
        drop(self.cmd_tx);
        match tokio::time::timeout(timeout, self.join).await {
            Ok(_) => {}
            Err(_) => tracing::warn!("DynamicSupervisor shutdown timed out"),
        }
    }
}

async fn dynamic_loop(mut cmd_rx: mpsc::Receiver<DynCmd>, shutdown_rx: watch::Receiver<bool>) {
    let mut children: HashMap<String, JoinHandle<()>> = HashMap::new();
    loop {
        children.retain(|_, jh| !jh.is_finished());
        tokio::select! {
            cmd = cmd_rx.recv() => match cmd {
                Some(DynCmd::Add(spec)) => {
                    let sr = shutdown_rx.clone();
                    let id = spec.id.clone();
                    let jh = tokio::spawn(run_child_monitored(spec.id, spec.config, spec.factory, sr));
                    children.insert(id, jh);
                }
                Some(DynCmd::Remove(id)) => {
                    if let Some(jh) = children.remove(&id) { jh.abort(); }
                }
                None => { for (_, jh) in children.drain() { jh.abort(); } return; }
            }
        }
    }
}

// ── OneForOne ─────────────────────────────────────────────────────────────────

async fn run_one_for_one(specs: Vec<ChildSpec>, shutdown_rx: watch::Receiver<bool>) {
    let handles: Vec<_> = specs.into_iter().map(|spec| {
        let sr = shutdown_rx.clone();
        tokio::spawn(run_child_monitored(spec.id, spec.config, spec.factory, sr))
    }).collect();
    for h in handles { let _ = h.await; }
}

// ── OneForAll ─────────────────────────────────────────────────────────────────

async fn run_one_for_all(specs: Vec<ChildSpec>, mut shutdown_rx: watch::Receiver<bool>) {
    let mut intensity: HashMap<String, Vec<Instant>> = HashMap::new();
    loop {
        if *shutdown_rx.borrow() { return; }
        let (kill_tx, _) = watch::channel(false);
        let (crash_tx, mut crash_rx) = mpsc::unbounded_channel::<String>();
        let handles: Vec<JoinHandle<()>> = specs.iter().map(|spec| {
            let id      = spec.id.clone();
            let factory = Arc::clone(&spec.factory);
            let config  = spec.config.clone();
            let tx      = crash_tx.clone();
            let kill    = kill_tx.subscribe();
            let sr      = shutdown_rx.clone();
            tokio::spawn(one_for_all_child(id, config, factory, tx, kill, sr))
        }).collect();
        drop(crash_tx);

        let crashed = tokio::select! {
            maybe = crash_rx.recv() => maybe,
            _ = wait_shutdown(&mut shutdown_rx) => None,
        };
        match crashed {
            None => {
                let _ = kill_tx.send(true);
                for h in handles { h.abort(); }
                return;
            }
            Some(crashed_id) => {
                tracing::info!(crashed = %crashed_id, "one_for_all: restarting all children");
                let _ = kill_tx.send(true);
                for h in handles { h.abort(); }
                let now = Instant::now();
                let times = intensity.entry(crashed_id.clone()).or_default();
                times.retain(|t| now.duration_since(*t) < Duration::from_secs(5));
                if let Some(cfg) = specs.iter().find(|s| s.id == crashed_id).map(|s| &s.config) {
                    if times.len() >= cfg.max_restarts as usize {
                        tracing::error!(child = %crashed_id, "max restarts exceeded — terminating");
                        return;
                    }
                    times.push(now);
                    tokio::time::sleep(cfg.backoff).await;
                }
            }
        }
    }
}

async fn one_for_all_child(
    id: String, config: RestartConfig, factory: Factory,
    crash_tx: mpsc::UnboundedSender<String>,
    mut kill: watch::Receiver<bool>,
    mut shutdown: watch::Receiver<bool>,
) {
    let f = Arc::clone(&factory);
    let handle: JoinHandle<()> = tokio::spawn(async move { f().await });
    let abort = handle.abort_handle();
    let exit_normal = tokio::select! {
        result = handle => matches!(result, Ok(())),
        _ = wait_shutdown(&mut kill)     => { abort.abort(); return; }
        _ = wait_shutdown(&mut shutdown) => { abort.abort(); return; }
    };
    let should_restart = match config.policy {
        RestartPolicy::Permanent => true,
        RestartPolicy::Transient => !exit_normal,
        RestartPolicy::Temporary => false,
    };
    if should_restart { let _ = crash_tx.send(id); }
}

// ── Shared: per-child monitor (OneForOne / Dynamic) ───────────────────────────

async fn run_child_monitored(
    id: String, config: RestartConfig, factory: Factory,
    mut shutdown_rx: watch::Receiver<bool>,
) {
    let mut restart_times: Vec<Instant> = Vec::new();
    loop {
        if *shutdown_rx.borrow() { return; }
        let f = Arc::clone(&factory);
        let handle: JoinHandle<()> = tokio::spawn(async move { f().await });
        let abort = handle.abort_handle();
        let exit_normal = tokio::select! {
            result = handle => match result {
                Ok(())                     => { tracing::debug!(child=%id, "exited normally"); true }
                Err(ref e) if e.is_panic() => { tracing::error!(child=%id, "panicked: {e:?}"); false }
                Err(ref e)                 => { tracing::warn!(child=%id, "cancelled: {e:?}"); false }
            },
            _ = wait_shutdown(&mut shutdown_rx) => { abort.abort(); return; }
        };
        let should_restart = match config.policy {
            RestartPolicy::Permanent => true,
            RestartPolicy::Transient => !exit_normal,
            RestartPolicy::Temporary => false,
        };
        if !should_restart {
            tracing::debug!(child=%id, policy=?config.policy, "will not restart");
            return;
        }
        let now = Instant::now();
        restart_times.retain(|t| now.duration_since(*t) < config.window);
        if restart_times.len() >= config.max_restarts as usize {
            tracing::error!(child=%id, max_restarts=config.max_restarts, "max restart intensity exceeded");
            return;
        }
        restart_times.push(now);
        tracing::info!(child=%id, attempt=restart_times.len(), backoff_ms=config.backoff.as_millis(), "restarting child");
        tokio::time::sleep(config.backoff).await;
    }
}

async fn wait_shutdown(rx: &mut watch::Receiver<bool>) {
    loop {
        if *rx.borrow() { return; }
        if rx.changed().await.is_err() { return; }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    fn counter() -> (Arc<AtomicU32>, Arc<AtomicU32>) {
        let c = Arc::new(AtomicU32::new(0));
        (Arc::clone(&c), c)
    }

    #[tokio::test]
    async fn temporary_runs_once() {
        let (c, ctr) = counter();
        Supervisor::one_for_one()
            .child(ChildSpec::new("t", RestartConfig::temporary(), move || {
                let c = Arc::clone(&c);
                Box::pin(async move { c.fetch_add(1, Ordering::SeqCst); })
            }))
            .start().wait().await;
        assert_eq!(ctr.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn transient_not_restarted_on_clean() {
        let (c, ctr) = counter();
        Supervisor::one_for_one()
            .child(ChildSpec::new("t", RestartConfig::transient(), move || {
                let c = Arc::clone(&c);
                Box::pin(async move { c.fetch_add(1, Ordering::SeqCst); })
            }))
            .start().wait().await;
        assert_eq!(ctr.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn transient_restarted_on_panic() {
        let (c, ctr) = counter();
        Supervisor::one_for_one()
            .child(ChildSpec::new(
                "t",
                RestartConfig::transient().with_backoff(Duration::from_millis(1)),
                move || {
                    let c = Arc::clone(&c);
                    Box::pin(async move {
                        if c.fetch_add(1, Ordering::SeqCst) < 2 { panic!("test"); }
                    })
                },
            ))
            .start().wait().await;
        assert_eq!(ctr.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn max_restarts_stops_child() {
        let (c, ctr) = counter();
        Supervisor::one_for_one()
            .child(ChildSpec::new(
                "t",
                RestartConfig::permanent()
                    .with_backoff(Duration::from_millis(1))
                    .with_intensity(2, Duration::from_secs(60)),
                move || {
                    let c = Arc::clone(&c);
                    Box::pin(async move { c.fetch_add(1, Ordering::SeqCst); panic!("test"); })
                },
            ))
            .start().wait().await;
        assert_eq!(ctr.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn shutdown_stops_running_child() {
        let (c, ctr) = counter();
        let h = Supervisor::one_for_one()
            .child(ChildSpec::new("t", RestartConfig::permanent(), move || {
                let c = Arc::clone(&c);
                Box::pin(async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_secs(60)).await;
                })
            }))
            .start();
        tokio::time::sleep(Duration::from_millis(20)).await;
        h.shutdown(Duration::from_millis(200)).await;
        assert_eq!(ctr.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn one_for_all_restarts_siblings() {
        let (ca, ctr_a) = counter();
        let (cb, ctr_b) = counter();
        Supervisor::one_for_all()
            .child(ChildSpec::new(
                "a",
                RestartConfig::transient().with_backoff(Duration::from_millis(1)),
                move || {
                    let ca = Arc::clone(&ca);
                    Box::pin(async move {
                        if ca.fetch_add(1, Ordering::SeqCst) == 0 { panic!("first run"); }
                    })
                },
            ))
            .child(ChildSpec::new(
                "b",
                RestartConfig::permanent().with_backoff(Duration::from_millis(1)),
                move || {
                    let cb = Arc::clone(&cb);
                    Box::pin(async move { cb.fetch_add(1, Ordering::SeqCst); })
                },
            ))
            .start().wait().await;
        assert!(ctr_a.load(Ordering::SeqCst) >= 2, "a restarted after panic");
        assert!(ctr_b.load(Ordering::SeqCst) >= 2, "b restarted via one_for_all");
    }

    #[tokio::test]
    async fn dynamic_add_and_shutdown() {
        let (c, ctr) = counter();
        let dyn_sup = DynamicSupervisor::start();
        dyn_sup.add_child(ChildSpec::new("w", RestartConfig::permanent(), move || {
            let c = Arc::clone(&c);
            Box::pin(async move {
                c.fetch_add(1, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_secs(60)).await;
            })
        })).await;
        tokio::time::sleep(Duration::from_millis(30)).await;
        dyn_sup.shutdown(Duration::from_millis(200)).await;
        assert_eq!(ctr.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn dynamic_terminate_child() {
        let (c, ctr) = counter();
        let dyn_sup = DynamicSupervisor::start();
        dyn_sup.add_child(ChildSpec::new("w", RestartConfig::permanent(), move || {
            let c = Arc::clone(&c);
            Box::pin(async move {
                c.fetch_add(1, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_secs(60)).await;
            })
        })).await;
        tokio::time::sleep(Duration::from_millis(30)).await;
        dyn_sup.terminate_child("w").await;
        tokio::time::sleep(Duration::from_millis(30)).await;
        dyn_sup.shutdown(Duration::from_millis(200)).await;
        assert_eq!(ctr.load(Ordering::SeqCst), 1);
    }
}
