mod state;
mod models;
mod templates;
mod controllers;
mod plugs;

use aethos::{router, Logger, BodyParser, SecureHeaders};
use axum::Extension;
use tower_http::services::ServeDir;
use std::path::PathBuf;

use state::AppState;
use plugs::FetchState;
use controllers::PageController;

fn build_router(state: AppState) -> axum::Router {
    router! {
        pipeline :browser {
            plug!(Logger);
            plug!(BodyParser);
            plug!(SecureHeaders);
            plug!(FetchState);
        }

        scope "/" {
            pipe_through!(:browser);
            get!("/", PageController, index);
            get!("/wiki/new", PageController, new_form);
            post!("/wiki/new", PageController, create);
            get!("/wiki/:title/edit", PageController, edit_form);
            post!("/wiki/:title/edit", PageController, update);
            get!("/wiki/:title", PageController, show);
            get!("/search", PageController, search);
            get!("/random", PageController, random);
        }
    }
    .nest_service("/assets", ServeDir::new(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")))
    .layer(Extension(state))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let content_dir = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/content"));
    let state       = AppState { content_dir };
    let router      = build_router(state);

    let addr     = "127.0.0.1:4000";
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("Wiki running at http://{}", addr);
    axum::serve(listener, router).await?;
    Ok(())
}
