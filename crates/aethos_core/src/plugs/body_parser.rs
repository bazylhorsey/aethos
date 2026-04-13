use async_trait::async_trait;
use axum::body::Body;
use http::header;
use http_body_util::BodyExt;
use tracing::debug;

use crate::{Conn, Next, Plug};

/// Parses request bodies (JSON and form-encoded) and merges the fields into
/// `conn.params`, making them available alongside path and query params.
#[derive(Default)]
pub struct BodyParser;

#[async_trait]
impl Plug for BodyParser {
    async fn call(&self, mut conn: Conn, next: Next) -> Conn {
        let content_type = conn.request.headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(str::to_lowercase)
            .unwrap_or_default();

        // Consume the body bytes
        let body = std::mem::replace(conn.request.body_mut(), Body::empty());
        match body.collect().await {
            Err(e) => {
                debug!("BodyParser: failed to read body: {e}");
            }
            Ok(collected) => {
                let bytes = collected.to_bytes();
                if !bytes.is_empty() {
                    if content_type.contains("application/json") {
                        if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                            if let Some(obj) = val.as_object() {
                                for (k, v) in obj {
                                    let s = match v {
                                        serde_json::Value::String(s) => s.clone(),
                                        other => other.to_string(),
                                    };
                                    conn.params.insert(k.clone(), s);
                                }
                            }
                        }
                    } else if content_type.contains("application/x-www-form-urlencoded") {
                        if let Ok(s) = std::str::from_utf8(&bytes) {
                            for pair in s.split('&') {
                                let mut parts = pair.splitn(2, '=');
                                let k = parts.next().unwrap_or("").trim();
                                let v = parts.next().unwrap_or("");
                                if !k.is_empty() {
                                    conn.params.insert(url_decode(k), url_decode(v));
                                }
                            }
                        }
                    }
                }
            }
        }

        next.run(conn).await
    }
}

fn url_decode(s: &str) -> String {
    let s = s.replace('+', " ");
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            let h: String = chars.by_ref().take(2).collect();
            if h.len() == 2 {
                if let Ok(b) = u8::from_str_radix(&h, 16) {
                    out.push(b as char);
                    continue;
                }
            }
            out.push('%');
            out.push_str(&h);
        } else {
            out.push(c);
        }
    }
    out
}
