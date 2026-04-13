use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use aethos_channels::Socket;
use aethos_pubsub::{Message, PubSub};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::debug;

/// Metadata attached to a presence entry.
pub type Meta = Value;

/// All entries for a single tracked key (user_id).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresenceEntry {
    pub metas: Vec<Meta>,
}

type TopicMap = HashMap<String, HashMap<String, PresenceEntry>>;

/// In-process presence tracking for a single node.
///
/// Tracks who is on what topic and broadcasts `presence_state` / `presence_diff`
/// events via PubSub so that channels and LiveViews can react.
#[derive(Clone, Default)]
pub struct Presence {
    state: Arc<Mutex<TopicMap>>,
    pubsub: PubSub,
}

impl Presence {
    pub fn new(pubsub: PubSub) -> Self {
        Self {
            state: Arc::new(Mutex::new(HashMap::new())),
            pubsub,
        }
    }

    /// Track a key on the socket's topic with the given metadata.
    pub fn track(&self, socket: &Socket, key: impl Into<String>, meta: Meta) {
        let topic = socket.topic.clone();
        let key = key.into();
        let mut state = self.state.lock().unwrap();
        let entries = state.entry(topic.clone()).or_default();
        entries
            .entry(key.clone())
            .or_insert_with(|| PresenceEntry { metas: Vec::new() })
            .metas
            .push(meta.clone());

        debug!(topic = %topic, key = %key, "Presence: track");
        self.pubsub.broadcast(
            &topic,
            Message::new(
                "presence_diff",
                serde_json::json!({
                    "joins": { &key: { "metas": [meta] } },
                    "leaves": {}
                }),
            ),
        );
    }

    /// Untrack a key on the socket's topic.
    pub fn untrack(&self, socket: &Socket, key: &str) {
        let topic = socket.topic.clone();
        let mut state = self.state.lock().unwrap();
        if let Some(entries) = state.get_mut(&topic) {
            if let Some(entry) = entries.remove(key) {
                debug!(topic = %topic, key = %key, "Presence: untrack");
                self.pubsub.broadcast(
                    &topic,
                    Message::new(
                        "presence_diff",
                        serde_json::json!({
                            "joins": {},
                            "leaves": { key: entry }
                        }),
                    ),
                );
            }
        }
    }

    /// Return the full presence list for a topic.
    pub fn list(&self, topic: &str) -> HashMap<String, PresenceEntry> {
        let state = self.state.lock().unwrap();
        state.get(topic).cloned().unwrap_or_default()
    }
}
