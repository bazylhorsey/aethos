use std::collections::HashMap;

/// Merged params: path captures + query string + body form fields.
#[derive(Default, Debug, Clone)]
pub struct Params(HashMap<String, String>);

impl Params {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(String::as_str)
    }

    pub fn insert(&mut self, key: impl Into<String>, val: impl Into<String>) {
        self.0.insert(key.into(), val.into());
    }

    pub fn extend_map(&mut self, map: HashMap<String, String>) {
        self.0.extend(map);
    }

    pub fn inner(&self) -> &HashMap<String, String> {
        &self.0
    }
}
