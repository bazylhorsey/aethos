use axum::{Router, response::{IntoResponse, Response}};
use http::{header, StatusCode, HeaderValue};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tower_http::services::ServeDir;
use tracing::info;

/// The compiled aethos.js client bundle, embedded at build time.
const AETHOS_JS: &str = include_str!("../../aethos/src/js/aethos.js");

/// Top-level application entry point.
///
/// Wraps an `axum::Router`, injects framework assets (`/_aethos/aethos.js`),
/// and initializes tracing. Analogous to `Phoenix.Endpoint`.
pub struct Endpoint {
    router: Router,
}

impl Endpoint {
    pub fn new(router: Router) -> Self {
        let router = router.route("/_aethos/aethos.js", axum::routing::get(serve_aethos_js));
        Self { router }
    }

    /// Serve static files from `dir` at `url_prefix`.
    ///
    /// Equivalent to `Plug.Static` in Phoenix.
    ///
    /// ```rust,ignore
    /// Endpoint::new(router)
    ///     .serve_static("/assets", "priv/static/assets")
    ///     .start(addr)
    ///     .await?;
    /// ```
    pub fn serve_static(mut self, url_prefix: &str, dir: &str) -> Self {
        self.router = self.router.nest_service(url_prefix, ServeDir::new(dir));
        self
    }

    /// Start listening on `addr`. This is async and runs until the process exits.
    pub async fn start(self, addr: SocketAddr) -> Result<(), Box<dyn std::error::Error>> {
        use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

        tracing_subscriber::registry()
            .with(tracing_subscriber::EnvFilter::new(
                std::env::var("AETHOS_LOG").unwrap_or_else(|_| "info".into()),
            ))
            .with(tracing_subscriber::fmt::layer())
            .init();

        let listener = TcpListener::bind(addr).await?;
        info!("Aethos listening on http://{}", addr);

        axum::serve(listener, self.router).await?;
        Ok(())
    }
}

async fn serve_aethos_js() -> Response {
    let mut resp = (StatusCode::OK, AETHOS_JS).into_response();
    resp.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/javascript; charset=utf-8"),
    );
    resp.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=86400"),
    );
    resp
}
