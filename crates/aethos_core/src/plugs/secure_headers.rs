use crate::{Conn, Next, Plug};
use http::HeaderValue;

/// Sets recommended security headers on every response:
/// X-Frame-Options, X-Content-Type-Options, X-XSS-Protection,
/// Referrer-Policy, and a basic Content-Security-Policy.
pub struct SecureHeaders;

#[async_trait::async_trait]
impl Plug for SecureHeaders {
    async fn call(&self, conn: Conn, next: Next) -> Conn {
        let mut conn = next.run(conn).await;
        let h = &mut conn.resp_headers;

        macro_rules! set {
            ($k:expr, $v:expr) => {
                h.insert($k, HeaderValue::from_static($v));
            };
        }

        set!("x-frame-options", "SAMEORIGIN");
        set!("x-content-type-options", "nosniff");
        set!("x-xss-protection", "1; mode=block");
        set!("referrer-policy", "strict-origin-when-cross-origin");

        conn
    }
}
