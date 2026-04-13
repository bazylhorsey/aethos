use std::collections::HashMap;
use serde_json::Value;

/// Flash messages (key → message), survive a single redirect.
#[derive(Default, Debug, Clone)]
pub struct FlashMap(HashMap<String, String>);

impl FlashMap {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn put(&mut self, key: impl Into<String>, msg: impl Into<String>) {
        self.0.insert(key.into(), msg.into());
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(String::as_str)
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Serialize to a JSON Value for session storage.
    pub fn to_json(&self) -> Value {
        serde_json::to_value(&self.0).unwrap_or(Value::Null)
    }

    /// Deserialize from a JSON Value retrieved from the session.
    pub fn from_json(val: &Value) -> Self {
        let map: HashMap<String, String> =
            serde_json::from_value(val.clone()).unwrap_or_default();
        Self(map)
    }

    pub fn inner(&self) -> &HashMap<String, String> {
        &self.0
    }
}
