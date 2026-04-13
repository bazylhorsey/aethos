use serde_json::Value;

/// The diff between two render outputs — only changed dynamic slots are included.
/// Sent as a JSON payload over the LiveView WebSocket connection.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct Diff {
    /// Changed dynamic slots: slot_id → new value.
    #[serde(skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub s: std::collections::HashMap<u32, Value>,
}

impl Diff {
    pub fn is_empty(&self) -> bool {
        self.s.is_empty()
    }

    pub fn insert(&mut self, id: u32, val: Value) {
        self.s.insert(id, val);
    }
}
