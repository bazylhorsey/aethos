use std::sync::Arc;
use aethos_core::TypeMap;

/// State carried by a LiveView process, analogous to Phoenix's `Socket`.
#[derive(Clone, Default)]
pub struct LiveSocket {
    /// Typed assigns — read by the `render` function.
    pub assigns: Arc<std::sync::Mutex<TypeMap>>,
    /// Whether the socket is connected (vs. initial static render).
    pub connected: bool,
}

impl LiveSocket {
    pub fn new(connected: bool) -> Self {
        Self {
            assigns: Arc::new(std::sync::Mutex::new(TypeMap::new())),
            connected,
        }
    }

    pub fn assign<T: std::any::Any + Send + Sync>(self, val: T) -> Self {
        self.assigns.lock().unwrap().insert(val);
        self
    }

    pub fn get_assign<T: std::any::Any + Send + Sync + Clone>(&self) -> Option<T> {
        self.assigns.lock().unwrap().get::<T>().cloned()
    }

    /// Update a typed assign in place.
    pub fn update<T: std::any::Any + Send + Sync, F: FnOnce(T) -> T>(self, f: F) -> Self
    where
        T: Default,
    {
        let val = self.assigns.lock().unwrap().remove::<T>().unwrap_or_default();
        self.assigns.lock().unwrap().insert(f(val));
        self
    }
}
