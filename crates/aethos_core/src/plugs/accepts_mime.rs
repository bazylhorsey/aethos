use async_trait::async_trait;
use http::StatusCode;
use crate::{Conn, plug::{Next, Plug}};

/// Validates that the request's `Accept` header includes at least one of the
/// allowed MIME types. Returns **406 Not Acceptable** if none match.
///
/// Phoenix / Plug equivalent: `plug :accepts, ["html", "json"]`.
///
/// # Shorthand MIME aliases
/// - `"html"` → `text/html`
/// - `"json"` → `application/json`
/// - `"text"` → `text/plain`
/// - `"xml"`  → `application/xml`
/// - anything containing `/` is treated as a full MIME type
///
/// If the request has no `Accept` header, it is allowed through (matches all).
///
/// ```rust,ignore
/// pipeline :browser {
///     plug!(AcceptsMime, ["html"]);
/// }
/// pipeline :api {
///     plug!(AcceptsMime, ["json"]);
/// }
/// ```
pub struct AcceptsMime {
    accepted: Vec<String>,
}

impl AcceptsMime {
    pub fn new(types: &[&str]) -> Self {
        Self {
            accepted: types.iter().map(|s| expand_mime(s)).collect(),
        }
    }
}

impl Default for AcceptsMime {
    fn default() -> Self {
        Self::new(&["html", "json", "text"])
    }
}

#[async_trait]
impl Plug for AcceptsMime {
    async fn call(&self, conn: Conn, next: Next) -> Conn {
        let accept = conn
            .request
            .headers()
            .get(http::header::ACCEPT)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("*/*");

        // */* means "accept anything"
        if accept.contains("*/*") {
            return next.run(conn).await;
        }

        let accepts_one = self.accepted.iter().any(|mime| accept.contains(mime.as_str()));
        if accepts_one {
            next.run(conn).await
        } else {
            tracing::warn!(
                "406 Not Acceptable: Accept={accept:?}, allowed={:?}",
                self.accepted
            );
            conn.put_status(StatusCode::NOT_ACCEPTABLE)
                .text("Not Acceptable")
                .halt()
        }
    }
}

fn expand_mime(s: &str) -> String {
    match s {
        "html"    => "text/html".into(),
        "json"    => "application/json".into(),
        "text"    => "text/plain".into(),
        "xml"     => "application/xml".into(),
        "form"    => "application/x-www-form-urlencoded".into(),
        "multipart" => "multipart/form-data".into(),
        other if other.contains('/') => other.into(),
        other     => other.into(),
    }
}
