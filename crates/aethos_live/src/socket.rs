use aethos_core::TypeMap;
use serde_json::Value;

/// A pending client-side navigation action.
#[derive(Clone, Debug)]
pub enum NavigationAction {
    /// Full navigation to a new URL (replaces the LiveView process).
    Navigate(String),
    /// Patch current URL (updates params without remounting).
    Patch(String),
}

/// A single stream item operation.
#[derive(Clone, Debug)]
pub enum StreamOp {
    Insert { name: String, id: String, item: Value },
    Delete { name: String, id: String },
    Reset  { name: String },
}

/// A pending flash message.
#[derive(Clone, Debug)]
pub struct FlashMsg {
    pub key: String,
    pub msg: String,
}

/// State carried by a LiveView process, analogous to Phoenix's `Socket`.
///
/// `LiveSocket` is moved through `mount`, `handle_event`, and `handle_info`
/// as an **owned value** — no interior mutability needed.
#[derive(Default)]
pub struct LiveSocket {
    /// Typed assigns — read by the `render` function.
    pub assigns: TypeMap,
    /// Whether the socket is connected (vs. initial static render).
    pub connected: bool,
    /// Pending navigation set by `navigate()` or `patch()`.
    pub(crate) navigation: Option<NavigationAction>,
    /// Pending stream operations set by `stream_insert` / `stream_delete` / etc.
    pub(crate) stream_ops: Vec<StreamOp>,
    /// Pending flash messages to push to the client.
    pub(crate) flash_msgs: Vec<FlashMsg>,
}

impl LiveSocket {
    pub fn new(connected: bool) -> Self {
        Self {
            assigns: TypeMap::new(),
            connected,
            navigation: None,
            stream_ops: Vec::new(),
            flash_msgs: Vec::new(),
        }
    }

    pub fn assign<T: std::any::Any + Send + Sync>(mut self, val: T) -> Self {
        self.assigns.insert(val);
        self
    }

    pub fn get_assign<T: std::any::Any + Send + Sync + Clone>(&self) -> Option<T> {
        self.assigns.get::<T>().cloned()
    }

    /// Update a typed assign in place.
    pub fn update<T: std::any::Any + Send + Sync, F: FnOnce(T) -> T>(mut self, f: F) -> Self
    where
        T: Default,
    {
        let val = self.assigns.remove::<T>().unwrap_or_default();
        self.assigns.insert(f(val));
        self
    }

    // ── Navigation ────────────────────────────────────────────────────────────

    /// Navigate to a new URL. The browser will perform a full LiveView navigation,
    /// mounting a new LiveView process for the destination route.
    ///
    /// Analogous to `Phoenix.LiveView.push_navigate/2`.
    pub fn navigate(mut self, url: impl Into<String>) -> Self {
        self.navigation = Some(NavigationAction::Navigate(url.into()));
        self
    }

    /// Patch the current URL without remounting the LiveView. Updates the query
    /// params and triggers `handle_params`.
    ///
    /// Analogous to `Phoenix.LiveView.push_patch/2`.
    pub fn patch(mut self, url: impl Into<String>) -> Self {
        self.navigation = Some(NavigationAction::Patch(url.into()));
        self
    }

    // ── Flash ─────────────────────────────────────────────────────────────────

    /// Push a flash message to the client.
    ///
    /// Analogous to `Phoenix.LiveView.put_flash/3`.
    pub fn put_flash(mut self, key: impl Into<String>, msg: impl Into<String>) -> Self {
        self.flash_msgs.push(FlashMsg { key: key.into(), msg: msg.into() });
        self
    }

    // ── Streams ───────────────────────────────────────────────────────────────

    /// Insert or update an item in a named stream.
    ///
    /// Use `phx-update="stream"` on the container element in your template.
    /// Each item must have a unique `id` used for DOM keying.
    ///
    /// Analogous to `Phoenix.LiveView.stream_insert/3`.
    pub fn stream_insert(
        mut self,
        name: impl Into<String>,
        id: impl Into<String>,
        item: impl Into<Value>,
    ) -> Self {
        self.stream_ops.push(StreamOp::Insert {
            name: name.into(),
            id: id.into(),
            item: item.into(),
        });
        self
    }

    /// Remove an item from a named stream by id.
    ///
    /// Analogous to `Phoenix.LiveView.stream_delete/3`.
    pub fn stream_delete(mut self, name: impl Into<String>, id: impl Into<String>) -> Self {
        self.stream_ops.push(StreamOp::Delete {
            name: name.into(),
            id: id.into(),
        });
        self
    }

    /// Remove all items from a named stream.
    ///
    /// Analogous to `Phoenix.LiveView.stream_reset/2`.
    pub fn stream_reset(mut self, name: impl Into<String>) -> Self {
        self.stream_ops.push(StreamOp::Reset { name: name.into() });
        self
    }

    /// Drain and return any pending navigation action.
    pub(crate) fn take_navigation(&mut self) -> Option<NavigationAction> {
        self.navigation.take()
    }

    /// Drain and return any pending stream operations.
    pub(crate) fn take_stream_ops(&mut self) -> Vec<StreamOp> {
        std::mem::take(&mut self.stream_ops)
    }

    /// Drain and return any pending flash messages.
    pub(crate) fn take_flash_msgs(&mut self) -> Vec<FlashMsg> {
        std::mem::take(&mut self.flash_msgs)
    }
}

