use async_trait::async_trait;
use axum::body::Body;
use http::{Request, StatusCode};
use tower::ServiceExt;
use tower_http::services::ServeDir;
use crate::{Conn, plug::{Next, Plug}};

/// Serves static files from a directory on disk.
///
/// Equivalent to `Plug.Static` in Phoenix. Files are served from `root_dir`
/// and matched against requests with the given `url_prefix`.
///
/// ```rust,ignore
/// // Serve files from ./priv/static at /static/*
/// plug!(StaticFiles, "/static", "priv/static");
/// ```
///
/// Or wire it up in the Endpoint:
/// ```rust,ignore
/// Endpoint::new(router)
///     .serve_static("/assets", "priv/static/assets")
///     .start(addr)
///     .await?;
/// ```
pub struct StaticFiles {
    url_prefix: String,
    root_dir: String,
}

impl StaticFiles {
    pub fn new(url_prefix: impl Into<String>, root_dir: impl Into<String>) -> Self {
        Self {
            url_prefix: url_prefix.into(),
            root_dir: root_dir.into(),
        }
    }
}

impl Default for StaticFiles {
    fn default() -> Self {
        Self::new("/static", "priv/static")
    }
}

#[async_trait]
impl Plug for StaticFiles {
    async fn call(&self, conn: Conn, next: Next) -> Conn {
        let path = conn.request.uri().path().to_owned();

        if !path.starts_with(&self.url_prefix) {
            return next.run(conn).await;
        }

        // Strip the URL prefix to get the file path relative to root_dir
        let file_path = path
            .strip_prefix(&self.url_prefix)
            .unwrap_or(&path)
            .trim_start_matches('/');

        let full_path = format!("{}/{}", self.root_dir.trim_end_matches('/'), file_path);

        if tokio::fs::metadata(&full_path).await.is_ok() {
            // Delegate to tower-http's ServeDir
            let svc = ServeDir::new(&self.root_dir);
            // Rebuild request with stripped prefix
            let stripped_uri: http::Uri = format!("/{file_path}")
                .parse()
                .unwrap_or_else(|_| "/".parse().unwrap());

            let (mut parts, body) = conn.request.into_parts();
            parts.uri = stripped_uri;
            let req = Request::from_parts(parts, body);

            match svc.oneshot(req).await {
                Ok(resp) => {
                    // Convert the tower-http response into our Conn response format
                    let (parts, body) = resp.into_parts();
                    use http_body_util::BodyExt;
                    let bytes = body
                        .collect()
                        .await
                        .map(|c| c.to_bytes())
                        .unwrap_or_default();
                    let mut builder = http::Response::builder().status(parts.status);
                    for (k, v) in &parts.headers {
                        builder = builder.header(k, v);
                    }
                    // We need to return a Conn; we don't have one with the original request anymore.
                    // Build a dummy conn with the static file response baked in.
                    let dummy_req = Request::builder()
                        .uri("/")
                        .body(Body::empty())
                        .unwrap();
                    let mut resp_conn = Conn::new(dummy_req);
                    resp_conn.status = parts.status;
                    for (k, v) in &parts.headers {
                        resp_conn.resp_headers.insert(k.clone(), v.clone());
                    }
                    resp_conn.body = crate::ResponseBody::Bytes(bytes);
                    resp_conn.halted = true;
                    resp_conn
                }
                Err(_) => {
                    let dummy_req = Request::builder().uri("/").body(Body::empty()).unwrap();
                    Conn::new(dummy_req)
                        .put_status(StatusCode::NOT_FOUND)
                        .text("Not Found")
                }
            }
        } else {
            // File not found, continue to next plug (404 from router is fine)
            let (parts, body) = conn.request.into_parts();
            let req = Request::from_parts(parts, body);
            next.run(Conn::new(req)).await
        }
    }
}
