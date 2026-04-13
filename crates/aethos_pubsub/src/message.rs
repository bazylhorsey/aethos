use serde::{Deserialize, Serialize};
use serde_json::Value;

/// An event message sent over a PubSub topic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// The event name, e.g. `"new_msg"` or `"presence_diff"`.
    pub event: String,
    /// The event payload.
    pub payload: Value,
}

impl Message {
    pub fn new(event: impl Into<String>, payload: impl Serialize) -> Self {
        Self {
            event: event.into(),
            payload: serde_json::to_value(payload).unwrap_or(Value::Null),
        }
    }
}
