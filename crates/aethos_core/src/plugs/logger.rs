use crate::{Conn, Next, Plug};
use crate::telemetry::{Telemetry, elapsed_ms};
use std::collections::HashMap;

/// Logs each request: method, path, status, and elapsed time.
/// Also emits `aethos.request.start` and `aethos.request.stop` telemetry events.
#[derive(Default)]
pub struct Logger;

#[async_trait::async_trait]
impl Plug for Logger {
    async fn call(&self, conn: Conn, next: Next) -> Conn {
        let method = conn.request.method().to_string();
        let path = conn.request.uri().path().to_owned();
        let start = std::time::Instant::now();

        // Emit start event
        let mut start_meta = HashMap::new();
        start_meta.insert("method".into(), method.clone());
        start_meta.insert("path".into(), path.clone());
        Telemetry::execute("aethos.request.start", {
            let mut m = HashMap::new();
            m.insert("system_time".into(), crate::telemetry::system_time_ms());
            m
        }, start_meta);

        let conn = next.run(conn).await;

        let duration = elapsed_ms(start);
        let status = conn.status.as_u16().to_string();

        tracing::info!(
            method = %method,
            path = %path,
            status = %conn.status.as_u16(),
            elapsed_ms = duration as u64,
        );

        // Emit stop event
        let mut stop_meta = HashMap::new();
        stop_meta.insert("method".into(), method);
        stop_meta.insert("path".into(), path);
        stop_meta.insert("status".into(), status);
        Telemetry::duration("aethos.request.stop", duration, stop_meta);

        conn
    }
}
