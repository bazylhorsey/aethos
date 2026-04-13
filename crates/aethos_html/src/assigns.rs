use std::any::{Any, TypeId};
use std::collections::HashMap;

/// Typed assigns passed into function components — analogous to Phoenix's `assigns` map.
/// Values are accessed in `h!` templates via `@name`.
#[derive(Default)]
pub struct Assigns {
    typed: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
    named: HashMap<String, Box<dyn Any + Send + Sync>>,
}

impl Assigns {
    pub fn new() -> Self {
        Self {
            typed: HashMap::new(),
            named: HashMap::new(),
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
