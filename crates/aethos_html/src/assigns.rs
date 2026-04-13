use std::any::{Any, TypeId};
use std::collections::HashMap;

use crate::Html;

/// Typed assigns passed into function components — analogous to Phoenix's `assigns` map.
/// Values are accessed in `h!` templates via `@name`.
#[derive(Default)]
pub struct Assigns {
    typed: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
    named: HashMap<String, Box<dyn Any + Send + Sync>>,
    slots: HashMap<String, Html>,
}

impl Assigns {
    pub fn new() -> Self {
        Self {
            typed: HashMap::new(),
            named: HashMap::new(),
            slots: HashMap::new(),
        }
    }

    /// Insert a value keyed by its Rust type.
    pub fn put<T: Any + Send + Sync>(mut self, val: T) -> Self {
        self.typed.insert(TypeId::of::<T>(), Box::new(val));
        self
    }

    /// Insert a value by string name (accessible as `@name` in templates via `get_named`).
    pub fn set(mut self, key: impl Into<String>, val: impl Any + Send + Sync) -> Self {
        self.named.insert(key.into(), Box::new(val));
        self
    }

    /// Insert a named slot (used by `h!` for `<:slot_name>content</:slot_name>`).
    ///
    /// Inside components, render slots with `assigns.slot("header")` or
    /// `{raw(assigns.slot("inner_block"))}` in `h!`.
    pub fn put_slot(mut self, name: impl Into<String>, html: Html) -> Self {
        self.slots.insert(name.into(), html);
        self
    }

    /// Retrieve a named slot's rendered HTML (empty if not provided).
    pub fn slot(&self, name: &str) -> Html {
        self.slots.get(name).cloned().unwrap_or(Html(String::new()))
    }

    /// Returns `true` if the named slot was provided by the caller.
    pub fn has_slot(&self, name: &str) -> bool {
        self.slots.contains_key(name)
    }

    /// Retrieve a value by type.
    pub fn get<T: Any + Send + Sync>(&self) -> Option<&T> {
        self.typed.get(&TypeId::of::<T>())?.downcast_ref()
    }

    /// Retrieve a value by string name.
    pub fn get_named<T: Any + Send + Sync>(&self, key: &str) -> Option<&T> {
        self.named.get(key)?.downcast_ref()
    }

    /// Check if a named key exists.
    pub fn has(&self, key: &str) -> bool {
        self.named.contains_key(key)
    }
}

/// Convenience macro for building `Assigns`.
///
/// ```rust,ignore
/// let a = assigns! { title: "Hello".to_string(), count: 42usize };
/// ```
#[macro_export]
macro_rules! assigns {
    ( $($key:ident : $val:expr),* $(,)? ) => {{
        let a = ::aethos_html::Assigns::new();
        $( let a = a.put($val); )*
        a
    }};
}
