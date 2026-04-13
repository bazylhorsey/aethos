//! Telemetry — lightweight event system for instrumenting Aethos applications.
//!
//! Analogous to the `:telemetry` library in the Phoenix ecosystem.
//!
//! # Usage
//!
//! ```rust,ignore
//! use aethos::telemetry::{Telemetry, Event};
//!
//! // Register a handler for all request-stop events
//! Telemetry::register("aethos.request.stop", |event| {
//!     println!("Request completed in {}ms: {} {}",
//!         event.measurements["duration_ms"],
//!         event.metadata["method"],
//!         event.metadata["path"],
//!     );
//! });
//! ```
//!
//! # Built-in events
//!
//! | Event name                      | When                          | Measurements        | Metadata                         |
//! |---------------------------------|-------------------------------|---------------------|----------------------------------|
//! | `aethos.request.start`          | Request received              | `system_time`       | `method`, `path`                 |
//! | `aethos.request.stop`           | Response sent                 | `duration_ms`       | `method`, `path`, `status`       |
//! | `aethos.live_view.mount`        | LiveView mounted              | `system_time`       | `view`                           |
//! | `aethos.live_view.handle_event` | LiveView event handled        | `duration_ms`       | `view`, `event`                  |
//! | `aethos.channel.join`           | Channel joined                | `system_time`       | `channel`, `topic`               |
//! | `aethos.channel.handle_in`      | Channel inbound message       | `duration_ms`       | `channel`, `topic`, `event`      |

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// A single telemetry event.
#[derive(Clone, Debug)]
pub struct Event {
    /// Dot-separated event name (e.g. `"aethos.request.stop"`).
    pub name: String,
    /// Numeric measurements (e.g. `{"duration_ms": 42}`).
    pub measurements: HashMap<String, f64>,
    /// String metadata (e.g. `{"method": "GET", "path": "/users"}`).
    pub metadata: HashMap<String, String>,
}

impl Event {
    pub fn new(
        name: impl Into<String>,
        measurements: HashMap<String, f64>,
        metadata: HashMap<String, String>,
    ) -> Self {
        Self { name: name.into(), measurements, metadata }
    }
}

type Handler = Arc<dyn Fn(&Event) + Send + Sync + 'static>;

/// Global telemetry registry.
static REGISTRY: std::sync::OnceLock<Arc<RwLock<HashMap<String, Vec<Handler>>>>> =
    std::sync::OnceLock::new();

fn registry() -> &'static Arc<RwLock<HashMap<String, Vec<Handler>>>> {
    REGISTRY.get_or_init(|| Arc::new(RwLock::new(HashMap::new())))
}

/// Telemetry event system.
pub struct Telemetry;

impl Telemetry {
    /// Register a handler for a named event.
    ///
    /// Handlers are called synchronously when `execute` is called for a
    /// matching event name.
    pub fn on(event_name: impl Into<String>, handler: impl Fn(&Event) + Send + Sync + 'static) {
        let mut reg = registry().write().unwrap();
        reg.entry(event_name.into())
            .or_default()
            .push(Arc::new(handler));
    }

    /// Emit a telemetry event, calling all registered handlers.
    pub fn execute(
        event_name: impl Into<String>,
        measurements: HashMap<String, f64>,
        metadata: HashMap<String, String>,
    ) {
        let name = event_name.into();
        let event = Event::new(name.clone(), measurements, metadata);
        let reg = registry().read().unwrap();
        if let Some(handlers) = reg.get(&name) {
            for handler in handlers {
                handler(&event);
            }
        }
    }

    /// Convenience: execute with a single duration measurement.
    pub fn duration(event_name: impl Into<String>, duration_ms: f64, metadata: HashMap<String, String>) {
        let mut m = HashMap::new();
        m.insert("duration_ms".into(), duration_ms);
        Self::execute(event_name, m, metadata);
    }
}

/// Returns milliseconds elapsed since `start_ns` (nanoseconds from `Instant`).
pub fn elapsed_ms(start: std::time::Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}

/// Returns the current Unix time in milliseconds.
pub fn system_time_ms() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as f64)
        .unwrap_or(0.0)
}
