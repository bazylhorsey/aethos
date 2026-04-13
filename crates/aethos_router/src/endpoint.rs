use axum::Router;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tracing::info;

/// Top-level application entry point.
///
/// Wraps an `axum::Router` and adds framework-level defaults (tracing subscriber,
/// graceful shutdown, etc.). Analogous to `Phoenix.Endpoint`.
pub struct Endpoint {
    router: Router,
}

impl Endpoint {
    pub fn new(router: Router) -> Self {
        Self { router }
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
