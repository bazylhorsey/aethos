//! Phoenix-style supervisor for Tokio tasks.
//!
//! Mirrors the OTP supervisor behaviour:
//! - Each child runs in its own `tokio::task`.
//! - Panics and crashes are caught via `JoinHandle` inspection.
//! - Children are restarted according to their [`RestartPolicy`].
//! - Restart *intensity* is capped: if a child crashes more than
//!   `max_restarts` times within `window`, it is abandoned (just like
//!   Phoenix's `max_restarts` / `max_seconds` option).
//!
//! # Example
//!
//! ```rust,ignore
//! use aethos_core::supervisor::{Supervisor, ChildSpec, RestartConfig};
//! use std::time::Duration;
//!
//! let supervisor = Supervisor::new()
//!     .child(ChildSpec::new(
//!         "metrics-reporter",
//!         RestartConfig::permanent().with_backoff(Duration::from_secs(1)),
//!         || Box::pin(async { run_metrics_loop().await }),
//!     ))
//!     .child(ChildSpec::new(
//!         "cache-warmer",
//!         RestartConfig::transient(),
//!         || Box::pin(async { warm_cache().await }),
//!     ));
//!
//! // Runs all children; blocks until every child has terminated.
//! supervisor.start().await.unwrap();
//! ```

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::task::JoinHandle;

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

// ── Supervisor ────────────────────────────────────────────────────────────────

/// Phoenix-style `:one_for_one` supervisor.
///
/// Each registered child is monitored independently. A crash in one child
/// never affects the others (`:one_for_one` strategy). If you need
/// `:one_for_all` semantics, coordinate via shared state or a channel.
#[derive(Default)]
pub struct Supervisor {
    specs: Vec<ChildSpec>,
}

impl Supervisor {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a child with this supervisor.
    pub fn child(mut self, spec: ChildSpec) -> Self {
        self.specs.push(spec);
        self
    }

    /// Spawn all children. Each child gets its own monitor task.
    ///
    /// Returns a `JoinHandle` that completes when **all** children have
    /// terminated (either cleanly or by exhausting their restart budget).
    pub fn start(self) -> JoinHandle<()> {
        tokio::spawn(async move {
            let handles: Vec<JoinHandle<()>> = self
                .specs
                .into_iter()
                .map(|spec| {
                    tokio::spawn(run_child(spec.id, spec.config, spec.factory))
                })
                .collect();

            for h in handles {
                if let Err(e) = h.await {
                    tracing::error!("supervisor monitor task panicked: {e:?}");
                }
            }
        })
    }
}

// ── Internal: per-child monitor loop ─────────────────────────────────────────

async fn run_child(id: String, config: RestartConfig, factory: Factory) {
    // Sliding window of restart timestamps
    let mut restart_times: Vec<Instant> = Vec::new();

    loop {
        let f = Arc::clone(&factory);
        let handle: JoinHandle<()> = tokio::spawn(async move { f().await });

        let exit_normal = match handle.await {
            Ok(()) => {
                tracing::debug!(child = %id, "exited normally");
                true
            }
            Err(ref e) if e.is_panic() => {
                tracing::error!(child = %id, "panicked: {e:?}");
                false
            }
            Err(ref e) => {
                tracing::warn!(child = %id, "cancelled: {e:?}");
                false
            }
        };

        let should_restart = match config.policy {
            RestartPolicy::Permanent => true,
            RestartPolicy::Transient => !exit_normal,
            RestartPolicy::Temporary => false,
        };

        if !should_restart {
            tracing::debug!(child = %id, policy = ?config.policy, "will not restart");
            return;
        }

        // Evict timestamps outside the sliding window
        let now = Instant::now();
        restart_times.retain(|t| now.duration_since(*t) < config.window);

        if restart_times.len() >= config.max_restarts as usize {
            tracing::error!(
                child = %id,
                max_restarts = config.max_restarts,
                window_secs  = config.window.as_secs(),
                "max restart intensity exceeded — child will not be restarted"
            );
            return;
        }

        restart_times.push(now);
        tracing::info!(
            child    = %id,
            attempt  = restart_times.len(),
            backoff_ms = config.backoff.as_millis(),
            "restarting child"
        );
        tokio::time::sleep(config.backoff).await;
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    #[tokio::test]
    async fn temporary_child_runs_once() {
        let counter = Arc::new(AtomicU32::new(0));
        let c = Arc::clone(&counter);

        let spec = ChildSpec::new(
            "tmp",
            RestartConfig::temporary(),
            move || {
                let c = Arc::clone(&c);
                Box::pin(async move { c.fetch_add(1, Ordering::SeqCst); })
            },
        );

        Supervisor::new().child(spec).start().await.unwrap();
        assert_eq!(counter.load(Ordering::SeqCst), 1, "temporary: runs exactly once");
    }

    #[tokio::test]
    async fn transient_child_not_restarted_on_clean_exit() {
        let counter = Arc::new(AtomicU32::new(0));
        let c = Arc::clone(&counter);

        let spec = ChildSpec::new(
            "transient-clean",
            RestartConfig::transient(),
            move || {
                let c = Arc::clone(&c);
                Box::pin(async move { c.fetch_add(1, Ordering::SeqCst); })
            },
        );

        Supervisor::new().child(spec).start().await.unwrap();
        assert_eq!(counter.load(Ordering::SeqCst), 1, "transient: not restarted on clean exit");
    }

    #[tokio::test]
    async fn transient_child_restarted_on_panic() {
        let counter = Arc::new(AtomicU32::new(0));
        let c = Arc::clone(&counter);

        // Panic twice, then exit cleanly.
        let spec = ChildSpec::new(
            "transient-panic",
            RestartConfig::transient().with_backoff(Duration::from_millis(1)),
            move || {
                let c = Arc::clone(&c);
                Box::pin(async move {
                    let n = c.fetch_add(1, Ordering::SeqCst);
                    if n < 2 { panic!("intentional test panic"); }
                })
            },
        );

        Supervisor::new().child(spec).start().await.unwrap();
        assert_eq!(counter.load(Ordering::SeqCst), 3, "transient: restarted after each panic");
    }

    #[tokio::test]
    async fn max_restarts_exceeded_stops_child() {
        let counter = Arc::new(AtomicU32::new(0));
        let c = Arc::clone(&counter);

        // Always panics; max_restarts = 2 → runs 3 times total (initial + 2 restarts).
        let spec = ChildSpec::new(
            "always-panic",
            RestartConfig::permanent()
                .with_backoff(Duration::from_millis(1))
                .with_intensity(2, Duration::from_secs(60)),
            move || {
                let c = Arc::clone(&c);
                Box::pin(async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    panic!("intentional");
                })
            },
        );

        Supervisor::new().child(spec).start().await.unwrap();
        // 1 initial run + 2 restarts = 3
        assert_eq!(counter.load(Ordering::SeqCst), 3, "stops after max_restarts exceeded");
    }
}
