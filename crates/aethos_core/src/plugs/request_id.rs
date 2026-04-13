use crate::{Conn, Next, Plug};
use http::HeaderValue;
use uuid::Uuid;

/// Attaches a unique `X-Request-Id` header to each request and response.
pub struct RequestId;

#[async_trait::async_trait]
impl Plug for RequestId {
    async fn call(&self, mut conn: Conn, next: Next) -> Conn {
        let id = Uuid::new_v4().to_string();
        if let Ok(v) = HeaderValue::from_str(&id) {
            conn.resp_headers.insert("x-request-id", v);
        }
        next.run(conn).await
    }
}
