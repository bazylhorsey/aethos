use async_trait::async_trait;
use http::StatusCode;
use rand::RngCore;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};

use crate::{Conn, plug::{Next, Plug}, crypto::constant_time_eq};

pub(crate) const CSRF_SESSION_KEY: &str = "_csrf_token";

/// Newtype stored in `conn.assigns` so templates can read the CSRF token.
#[derive(Clone)]
pub struct CsrfToken(pub String);

/// CSRF protection plug.
///
/// - On safe methods (GET, HEAD, OPTIONS): generates a token, stores it in the
///   session, and puts it in `conn.assigns` as `CsrfToken`.
/// - On mutating methods (POST, PUT, PATCH, DELETE): validates the token from
///   the `_csrf_token` param or `x-csrf-token` header. Returns **403** if
///   invalid.
///
/// Requires `FetchSession` to run first.
///
/// ```rust,ignore
/// pipeline :browser {
///     plug!(FetchSession);
///     plug!(Csrf);
/// }
/// ```
#[derive(Default)]
pub struct Csrf;

#[async_trait]
impl Plug for Csrf {
    async fn call(&self, mut conn: Conn, next: Next) -> Conn {
        let method = conn.request.method().clone();

        match method.as_str() {
            "GET" | "HEAD" | "OPTIONS" => {
                // Ensure a token exists in the session
                let token = match conn.session.get(CSRF_SESSION_KEY) {
                    Some(v) => v.as_str().unwrap_or("").to_string(),
                    None => {
                        let token = generate_token();
                        conn.session.put(CSRF_SESSION_KEY, token.clone());
                        token
                    }
                };
                conn.assigns.insert(CsrfToken(token));
                next.run(conn).await
            }

            "POST" | "PUT" | "PATCH" | "DELETE" => {
                let session_token = conn
                    .session
                    .get(CSRF_SESSION_KEY)
                    .and_then(|v| v.as_str())
                    .map(str::to_owned)
                    .unwrap_or_default();

                // Check param first, then header
                let provided = conn
                    .params
                    .get("_csrf_token")
                    .map(str::to_owned)
                    .or_else(|| {
                        conn.request
                            .headers()
                            .get("x-csrf-token")
                            .and_then(|v| v.to_str().ok())
                            .map(str::to_owned)
                    })
                    .unwrap_or_default();

                if constant_time_eq(session_token.as_bytes(), provided.as_bytes()) {
                    next.run(conn).await
                } else {
                    tracing::warn!("CSRF token mismatch — rejecting {method} request");
                    conn.put_status(StatusCode::FORBIDDEN)
                        .text("Invalid CSRF token")
                        .halt()
                }
            }

            _ => next.run(conn).await,
        }
    }
}

fn generate_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}
