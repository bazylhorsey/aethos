use crate::{Conn, Next, Plug};

/// Logs each request: method, path, status, and elapsed time.
#[derive(Default)]
pub struct Logger;

#[async_trait::async_trait]
impl Plug for Logger {
    async fn call(&self, conn: Conn, next: Next) -> Conn {
        let method = conn.request.method().clone();
        let path = conn.request.uri().path().to_owned();
        let start = std::time::Instant::now();

        let conn = next.run(conn).await;

        let elapsed = start.elapsed();
        tracing::info!(
            method = %method,
            path = %path,
            status = %conn.status.as_u16(),
            elapsed_ms = elapsed.as_millis(),
        );

        conn
    }
}
