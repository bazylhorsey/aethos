use axum::body::Body;
use axum::extract::Request;
use bytes::Bytes;
use http::{HeaderMap, HeaderValue, StatusCode, header};
use serde::Serialize;

use crate::{FlashMap, Params, ResponseBody, TypeMap, Session};

/// The central connection struct passed through every plug, analogous to `Plug.Conn`.
///
/// Plugs read from and write to `Conn`. The final `Conn` is converted into an `axum::Response`.
pub struct Conn {
    /// The incoming HTTP request.
    pub request: Request<Body>,

    /// Typed key-value store for data shared between plugs and controllers.
    pub assigns: TypeMap,

    /// Merged path captures + query params + body form fields.
    pub params: Params,

    /// Flash messages (survive one redirect via the session).
    pub flash: FlashMap,

    /// Cookie-backed session data, available after the `FetchSession` plug runs.
    pub session: Session,

    /// HTTP status code to send in the response.
    pub status: StatusCode,

    /// Response headers to send.
    pub resp_headers: HeaderMap,

    /// Response body accumulated by the controller/view.
    pub body: ResponseBody,

    /// When `true`, the plug chain is halted — no further plugs are invoked.
    pub halted: bool,

    /// Private framework data (not for application use).
    pub(crate) private: TypeMap,
}

impl Conn {
    /// Create a new `Conn` from an incoming Axum request.
    pub fn new(request: Request<Body>) -> Self {
        let mut params = Params::new();

        // Extract query string: ?foo=bar&baz=qux → params
        if let Some(query) = request.uri().query() {
            for pair in query.split('&') {
                let mut parts = pair.splitn(2, '=');
                let k = parts.next().unwrap_or("").trim();
                let v = parts.next().unwrap_or("");
                if !k.is_empty() {
                    params.insert(url_decode(k), url_decode(v));
                }
            }
        }

        Self {
            request,
            assigns: TypeMap::new(),
            params,
            flash: FlashMap::new(),
            session: Session::new(),
            status: StatusCode::OK,
            resp_headers: HeaderMap::new(),
            body: ResponseBody::Empty,
            halted: false,
            private: TypeMap::new(),
        }
    }

    // ── Assigns ──────────────────────────────────────────────────────────────

    /// Store a typed value in assigns.
    pub fn assign<T: std::any::Any + Send + Sync>(mut self, val: T) -> Self {
        self.assigns.insert(val);
        self
    }

    /// Retrieve a typed value from assigns.
    pub fn get_assign<T: std::any::Any + Send + Sync>(&self) -> Option<&T> {
        self.assigns.get::<T>()
    }

    // ── Flash ─────────────────────────────────────────────────────────────────

    pub fn put_flash(mut self, key: impl Into<String>, msg: impl Into<String>) -> Self {
        self.flash.put(key, msg);
        self
    }

    pub fn get_flash(&self, key: &str) -> Option<&str> {
        self.flash.get(key)
    }

    pub fn clear_flash(mut self) -> Self {
        self.flash.clear();
        self
    }

    // ── Halt ──────────────────────────────────────────────────────────────────

    /// Halt the plug chain. Subsequent plugs will not be called.
    pub fn halt(mut self) -> Self {
        self.halted = true;
        self
    }

    // ── Response helpers ──────────────────────────────────────────────────────

    pub fn put_status(mut self, status: StatusCode) -> Self {
        self.status = status;
        self
    }

    pub fn put_resp_header(mut self, key: &str, value: &str) -> Self {
        if let (Ok(k), Ok(v)) = (
            key.parse::<http::header::HeaderName>(),
            HeaderValue::from_str(value),
        ) {
            self.resp_headers.insert(k, v);
        }
        self
    }

    /// Send plain text response.
    pub fn text(mut self, body: impl Into<String>) -> Self {
        let s = body.into();
        self.resp_headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=utf-8"),
        );
        self.body = ResponseBody::Text(s);
        self.halted = true;
        self
    }

    /// Send JSON response, serializing `val`.
    pub fn json<T: Serialize>(mut self, val: &T) -> Self {
        match serde_json::to_string(val) {
            Ok(s) => {
                self.resp_headers.insert(
                    header::CONTENT_TYPE,
                    HeaderValue::from_static("application/json"),
                );
                self.body = ResponseBody::Text(s);
            }
            Err(e) => {
                tracing::error!("conn.json serialization error: {e}");
                self.status = StatusCode::INTERNAL_SERVER_ERROR;
                self.body = ResponseBody::Text("Internal Server Error".into());
            }
        }
        self.halted = true;
        self
    }

    /// Send raw HTML response.
    pub fn html(mut self, body: impl Into<String>) -> Self {
        self.resp_headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/html; charset=utf-8"),
        );
        self.body = ResponseBody::Text(body.into());
        self.halted = true;
        self
    }

    /// Send raw bytes response.
    pub fn send_resp(mut self, status: StatusCode, body: impl Into<Bytes>) -> Self {
        self.status = status;
        self.body = ResponseBody::Bytes(body.into());
        self.halted = true;
        self
    }

    /// Redirect to an internal path. Uses 302 by default.
    pub fn redirect(mut self, path: impl Into<String>) -> Self {
        let path = path.into();
        self.status = StatusCode::FOUND;
        if let Ok(v) = HeaderValue::from_str(&path) {
            self.resp_headers.insert(header::LOCATION, v);
        }
        self.body = ResponseBody::Empty;
        self.halted = true;
        self
    }

    /// Redirect with explicit status code (e.g. 301).
    pub fn redirect_with_status(mut self, path: impl Into<String>, status: StatusCode) -> Self {
        let path = path.into();
        self.status = status;
        if let Ok(v) = HeaderValue::from_str(&path) {
            self.resp_headers.insert(header::LOCATION, v);
        }
        self.body = ResponseBody::Empty;
        self.halted = true;
        self
    }

    // ── Status shortcuts ──────────────────────────────────────────────────────

    /// 200 OK with no body. Useful as a default before setting a body.
    pub fn ok(self) -> Self { self.put_status(StatusCode::OK) }

    /// 201 Created — body should describe the created resource.
    pub fn created(mut self, body: impl Into<String>) -> Self {
        self.status = StatusCode::CREATED;
        self.resp_headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        self.body = ResponseBody::Text(body.into());
        self.halted = true;
        self
    }

    /// 204 No Content.
    pub fn no_content(mut self) -> Self {
        self.status = StatusCode::NO_CONTENT;
        self.body = ResponseBody::Empty;
        self.halted = true;
        self
    }

    /// 400 Bad Request with a JSON `{"error": msg}` body.
    pub fn bad_request(self, msg: impl Into<String>) -> Self {
        let body = serde_json::json!({"error": msg.into()}).to_string();
        self.put_status(StatusCode::BAD_REQUEST)
            .put_resp_header("content-type", "application/json")
            .text(body)
    }

    /// 401 Unauthorized.
    pub fn unauthorized(self, msg: impl Into<String>) -> Self {
        let body = serde_json::json!({"error": msg.into()}).to_string();
        self.put_status(StatusCode::UNAUTHORIZED)
            .put_resp_header("content-type", "application/json")
            .text(body)
    }

    /// 403 Forbidden.
    pub fn forbidden(self, msg: impl Into<String>) -> Self {
        let body = serde_json::json!({"error": msg.into()}).to_string();
        self.put_status(StatusCode::FORBIDDEN)
            .put_resp_header("content-type", "application/json")
            .text(body)
    }

    /// 404 Not Found.
    pub fn not_found(self, msg: impl Into<String>) -> Self {
        let body = serde_json::json!({"error": msg.into()}).to_string();
        self.put_status(StatusCode::NOT_FOUND)
            .put_resp_header("content-type", "application/json")
            .text(body)
    }

    /// 422 Unprocessable Entity — used for validation errors.
    pub fn unprocessable_entity(self, msg: impl Into<String>) -> Self {
        let body = serde_json::json!({"error": msg.into()}).to_string();
        self.put_status(StatusCode::UNPROCESSABLE_ENTITY)
            .put_resp_header("content-type", "application/json")
            .text(body)
    }

    /// 500 Internal Server Error.
    pub fn internal_server_error(self, msg: impl Into<String>) -> Self {
        let body = serde_json::json!({"error": msg.into()}).to_string();
        self.put_status(StatusCode::INTERNAL_SERVER_ERROR)
            .put_resp_header("content-type", "application/json")
            .text(body)
    }

    /// Return the request's HTTP method as a string (may differ from the actual
    /// transport method after `MethodOverride` runs).
    pub fn method(&self) -> &http::Method {
        self.request.method()
    }

    /// Return the request's `Accept` header value, if any.
    pub fn accept(&self) -> Option<&str> {
        self.request
            .headers()
            .get(header::ACCEPT)
            .and_then(|v| v.to_str().ok())
    }

    /// Return the request URI path.
    pub fn path(&self) -> &str {
        self.request.uri().path()
    }

    // ── Conversion ────────────────────────────────────────────────────────────

    /// Convert the `Conn` into an `axum::Response`.
    pub fn into_response(self) -> axum::response::Response {
        use axum::response::Response;

        let body_bytes = self.body.into_bytes();
        let mut builder = http::Response::builder().status(self.status);

        for (k, v) in &self.resp_headers {
            builder = builder.header(k, v);
        }

        // Write session back to cookie if it was modified
        if self.session.is_dirty() {
            let cookie_val = self.session.encode();
            let cookie = format!(
                "_aethos_session={cookie_val}; Path=/; HttpOnly; SameSite=Lax"
            );
            if let Ok(v) = HeaderValue::from_str(&cookie) {
                builder = builder.header(header::SET_COOKIE, v);
            }
        }

        builder
            .body(Body::from(body_bytes))
            .unwrap_or_else(|_| {
                Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(Body::from("Internal Server Error"))
                    .unwrap()
            })
    }
}

fn url_decode(s: &str) -> String {
    // Replace '+' with space, then decode %XX sequences as UTF-8 bytes.
    let s = s.replace('+', " ");
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(hi), Some(lo)) = (hex_val(bytes[i+1]), hex_val(bytes[i+2])) {
                out.push((hi << 4) | lo);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned())
}

#[inline]
fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

impl std::fmt::Debug for Conn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Conn")
            .field("status", &self.status)
            .field("halted", &self.halted)
            .finish()
    }
}
