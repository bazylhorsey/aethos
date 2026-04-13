/// Aethos — Phoenix-inspired web framework for Rust
///
/// Re-exports all public types from the framework crates for ergonomic one-line imports.
pub use aethos_core::{
    Conn, Plug, Next, BoxPlug, Params, FlashMap, ResponseBody, AethosError, TypeMap,
    async_trait, axum, bytes, http, serde, serde_json, tower, tracing,
};

pub use aethos_router::{Pipeline, Endpoint};

pub use aethos_html::{Html, Assigns, html_escape};

pub use aethos_pubsub::{PubSub, Message};

pub use aethos_channels::{Socket, Channel};

pub use aethos_presence::Presence;

pub use aethos_live::{LiveView, LiveSocket};

// Built-in plugs
pub use aethos_core::plugs::{Logger, RequestId, SecureHeaders};

// Proc macros
pub use aethos_macros::{router, h, controller};

// Re-export assigns! and h! convenience macros
pub use aethos_html::assigns;
