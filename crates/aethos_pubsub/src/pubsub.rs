use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::sync::broadcast;
use tracing::{debug, warn};

use crate::Message;

const CHANNEL_CAPACITY: usize = 256;

type TopicSender = broadcast::Sender<Message>;

/// In-process PubSub backed by `tokio::sync::broadcast` channels.
///
/// Topics are created lazily on first subscribe or broadcast.
///
/// ```rust,ignore
/// let pubsub = PubSub::new();
/// let mut rx = pubsub.subscribe("room:lobby");
/// pubsub.broadcast("room:lobby", Message::new("new_msg", &payload)).await;
/// let msg = rx.recv().await.unwrap();
/// ```
#[derive(Clone, Default)]
pub struct PubSub {
    topics: Arc<Mutex<HashMap<String, TopicSender>>>,
}

impl PubSub {
    pub fn new() -> Self {
        Self::default()
    }

    /// Subscribe to a topic. Returns a `broadcast::Receiver` for incoming messages.
    pub fn subscribe(&self, topic: &str) -> broadcast::Receiver<Message> {
        let mut map = self.topics.lock().unwrap();
        let sender = map
            .entry(topic.to_owned())
            .or_insert_with(|| broadcast::channel(CHANNEL_CAPACITY).0);
        debug!(topic = %topic, "PubSub: new subscriber");
        sender.subscribe()
    }

    /// Broadcast a message to all subscribers on `topic`.
    pub fn broadcast(&self, topic: &str, msg: Message) {
        let map = self.topics.lock().unwrap();
        if let Some(sender) = map.get(topic) {
            if sender.receiver_count() == 0 {
                debug!(topic = %topic, "PubSub: broadcast with no subscribers");
                return;
            }
            if let Err(e) = sender.send(msg) {
                warn!(topic = %topic, err = %e, "PubSub broadcast error");
            }
        }
    }

    /// Number of active subscribers on `topic`.
    pub fn subscriber_count(&self, topic: &str) -> usize {
        let map = self.topics.lock().unwrap();
        map.get(topic)
            .map(|s| s.receiver_count())
            .unwrap_or(0)
    }
}
