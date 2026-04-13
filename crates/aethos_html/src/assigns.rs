use std::any::{Any, TypeId};
use std::collections::HashMap;

/// Typed assigns passed into function components — analogous to Phoenix's `assigns` map.
/// Values are accessed in `h!` templates via `@name`.
#[derive(Default)]
pub struct Assigns(HashMap<TypeId, Box<dyn Any + Send + Sync>>);

impl Assigns {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn put<T: Any + Send + Sync>(mut self, val: T) -> Self {
        self.0.insert(TypeId::of::<T>(), Box::new(val));
        self
    }

    pub fn get<T: Any + Send + Sync>(&self) -> Option<&T> {
        self.0.get(&TypeId::of::<T>())?.downcast_ref()
    }
}

/// Convenience macro for building `Assigns`.
///
/// ```rust
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
