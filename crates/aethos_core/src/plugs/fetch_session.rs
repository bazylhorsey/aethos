use async_trait::async_trait;
use crate::{Conn, plug::{Next, Plug}, Session};

const COOKIE_NAME: &str = "_aethos_session";

/// Decodes the session cookie before the action and re-encodes it afterwards.
///
/// Add to your `:browser` pipeline:
/// ```rust,ignore
/// pipeline :browser {
///     plug!(FetchSession);
/// }
/// ```
#[derive(Default)]
pub struct FetchSession;

#[async_trait]
impl Plug for FetchSession {
    async fn call(&self, mut conn: Conn, next: Next) -> Conn {
        // ── Read cookie ───────────────────────────────────────────────────────
        if let Some(cookie_header) = conn.request.headers().get("cookie") {
            if let Ok(cookie_str) = cookie_header.to_str() {
                for part in cookie_str.split(';') {
                    let part = part.trim();
                    if let Some(val) = part.strip_prefix(&format!("{COOKIE_NAME}=")) {
                        if let Some(session) = Session::decode(val) {
                            conn.session = session;
                        }
                        break;
                    }
                }
            }
        }

        next.run(conn).await
    }
}
