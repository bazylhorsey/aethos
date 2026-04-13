/// Aethos — Phoenix-inspired web framework for Rust
///
/// Re-exports all public types from the framework crates for ergonomic one-line imports.
pub use aethos_core::{
    Conn, Plug, Next, BoxPlug, Params, FlashMap, ResponseBody, AethosError, TypeMap,
    async_trait, axum, bytes, http, serde, serde_json, tower, tracing,
};

pub use aethos_router::{Pipeline, Endpoint};

pub use aethos_html::{Html, Assigns, html_escape, Safe, default_root_layout, InnerContent, ConnHtmlExt};

pub use aethos_pubsub::{PubSub, Message};

pub use aethos_channels::{Socket, Channel, handle_socket};
/// Re-export channel sub-types
pub mod channel {
    pub use aethos_channels::channel::{JoinResult, JoinError};
}

pub use aethos_presence::Presence;

pub use aethos_live::{LiveView, LiveSocket, handle_live_socket};

// Built-in plugs
pub use aethos_core::plugs::{Logger, RequestId, SecureHeaders, BodyParser};

// Proc macros
pub use aethos_macros::{router, h, controller};

// Re-export assigns! and h! convenience macros
pub use aethos_html::assigns;
