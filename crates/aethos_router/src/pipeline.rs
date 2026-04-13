use std::sync::Arc;
use aethos_core::{BoxPlug, Conn, Plug};

/// A named, ordered list of plugs — analogous to a Phoenix pipeline.
///
/// ```rust,ignore
/// let browser = Pipeline::new("browser")
///     .plug(Logger)
///     .plug(SecureHeaders);
/// ```
#[derive(Clone)]
pub struct Pipeline {
    pub name: &'static str,
    plugs: Vec<BoxPlug>,
}

impl Pipeline {
    pub fn new(name: &'static str) -> Self {
        Self { name, plugs: Vec::new() }
    }

    /// Append a plug to this pipeline.
    pub fn plug<P: Plug>(mut self, p: P) -> Self {
        self.plugs.push(Arc::new(p));
        self
    }

    /// Append a type-erased plug.
    pub fn plug_boxed(mut self, p: BoxPlug) -> Self {
        self.plugs.push(p);
        self
    }

    /// Run this pipeline against `conn`. If `conn.halted` is set by any plug,
    /// the chain stops and the halted `Conn` is returned.
    pub async fn run(&self, conn: Conn) -> Conn {
        let plugs = self.plugs.clone();
        aethos_core::plug::compose(plugs)(conn).await
    }
}
