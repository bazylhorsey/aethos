use async_trait::async_trait;
use axum::response::Response;
use serde_json::Value;

use aethos_core::Conn;
use aethos_html::Html;

use crate::LiveSocket;

/// Parameters map passed to `mount`.
pub type Params = std::collections::HashMap<String, String>;

/// The core LiveView trait. Implement this and register with `live!()` in the router.
///
/// Lifecycle:
/// 1. `mount` — initial state setup (called twice: static render + WS connect)
/// 2. `render` — returns an `Html` fragment from the current socket assigns
/// 3. `handle_event` — reacts to browser events (`phx-click`, `phx-submit`, etc.)
/// 4. `handle_info` — reacts to internal Tokio/PubSub messages
#[async_trait]
pub trait LiveView: Send + Sync + 'static {
    /// Initialize the LiveView state.
    async fn mount(params: Params, socket: LiveSocket) -> LiveSocket
    where
        Self: Sized;

    /// Render the current state into HTML.
    fn render(socket: &LiveSocket) -> Html
    where
        Self: Sized;

    /// Handle a browser event.
    async fn handle_event(_event: &str, _payload: Value, socket: LiveSocket) -> LiveSocket
    where
        Self: Sized,
    {
        socket
    }

    /// Handle URL parameter changes (called after `mount` and after `push_patch`).
    ///
    /// Analogous to `Phoenix.LiveView.handle_params/3`.
    async fn handle_params(
        _params: std::collections::HashMap<String, String>,
        _url: &str,
        socket: LiveSocket,
    ) -> LiveSocket
    where
        Self: Sized,
    {
        socket
    }

    /// Handle an internal message (from PubSub, timers, etc.)
    async fn handle_info(_msg: Value, socket: LiveSocket) -> LiveSocket
    where
        Self: Sized,
    {
        socket
    }

    /// Called by the router to handle an incoming HTTP request.
    /// Performs the initial static render wrapped in the LiveView container.
    async fn handle_request(&self, conn: Conn) -> Response
    where
        Self: Sized + Default + 'static,
    {
        let params = extract_params(&conn);
        let socket = LiveSocket::new(false);
        let socket = Self::mount(params, socket).await;
        let html   = Self::render(&socket);

        let page = format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <script type="module" src="/_aethos/aethos.js"></script>
</head>
<body>
  <div id="phx-root" data-phx-live>
    {}
  </div>
</body>
</html>"#,
            html.as_str()
        );

        conn.html(page).into_response()
    }
}

fn extract_params(conn: &Conn) -> Params {
    conn.params.inner().clone()
}
