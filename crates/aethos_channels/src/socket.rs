use std::sync::Arc;
use aethos_core::TypeMap;
use aethos_pubsub::{Message, PubSub};
use serde_json::Value;
use tokio::sync::mpsc;
use tracing::warn;

/// The socket state associated with a WebSocket connection / channel join.
/// Analogous to Phoenix's `Phoenix.Socket`.
#[derive(Clone)]
pub struct Socket {
    /// The channel topic, e.g. `"room:lobby"`.
    pub topic: String,

    /// Typed assigns on this socket (set with `.assign()`).
    pub assigns: Arc<std::sync::Mutex<TypeMap>>,

    /// Channel for sending messages back to the browser.
    pub(crate) reply_tx: mpsc::UnboundedSender<SocketMessage>,

    /// PubSub handle for broadcasting.
    pub(crate) pubsub: PubSub,
}

/// A raw message sent back to the WS client.
#[derive(Debug, Clone)]
pub struct SocketMessage {
    pub event: String,
    pub payload: Value,
    pub topic: String,
}

impl Socket {
    pub fn new_with_channel(
        topic: impl Into<String>,
        reply_tx: mpsc::UnboundedSender<SocketMessage>,
        pubsub: PubSub,
    ) -> Self {
        Self {
            topic: topic.into(),
            assigns: Arc::new(std::sync::Mutex::new(TypeMap::new())),
            reply_tx,
            pubsub,
        }
    }

    // ── Assigns ───────────────────────────────────────────────────────────────

    /// Store a typed value in socket assigns.
    pub fn assign<T: std::any::Any + Send + Sync>(self, val: T) -> Self {
        self.assigns.lock().unwrap().insert(val);
        self
    }

    /// Retrieve a typed value from socket assigns.
    pub fn get_assign<T: std::any::Any + Send + Sync + Clone>(&self) -> Option<T> {
        self.assigns.lock().unwrap().get::<T>().cloned()
    }

    // ── Messaging ─────────────────────────────────────────────────────────────

    /// Push an event directly to this socket's client.
    pub fn push(&self, event: impl Into<String>, payload: impl serde::Serialize) {
        let msg = SocketMessage {
            event: event.into(),
            payload: serde_json::to_value(payload).unwrap_or(Value::Null),
            topic: self.topic.clone(),
        };
        if let Err(e) = self.reply_tx.send(msg) {
            warn!("Socket push error: {e}");
        }
    }

    /// Broadcast an event to all subscribers on this socket's topic via PubSub.
    pub fn broadcast(&self, event: impl Into<String>, payload: impl serde::Serialize) -> &Self {
        let msg = Message::new(event, payload);
        self.pubsub.broadcast(&self.topic, msg);
        self
    }
}
