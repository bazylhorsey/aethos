use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::conn::Conn;

/// The continuation for the plug chain — calling `next.run(conn)` invokes the remaining plugs.
#[derive(Clone)]
pub struct Next(Arc<dyn Fn(Conn) -> BoxFuture<Conn> + Send + Sync>);

type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;

impl Next {
    pub fn new(f: impl Fn(Conn) -> BoxFuture<Conn> + Send + Sync + 'static) -> Self {
        Self(Arc::new(f))
    }

    pub async fn run(self, conn: Conn) -> Conn {
        (self.0)(conn).await
    }

    /// Terminal `Next` — returns conn unchanged (end of plug chain).
    pub fn terminal() -> Self {
        Self::new(|conn| Box::pin(async move { conn }))
    }
}

/// Core middleware abstraction, analogous to Phoenix's `Plug`.
///
/// A plug receives a `Conn`, may transform it, and either:
/// - calls `next.run(conn).await` to continue the chain, or
/// - returns the `Conn` directly (halting the chain, e.g. after setting `conn.halted = true`).
#[async_trait::async_trait]
pub trait Plug: Send + Sync + 'static {
    async fn call(&self, conn: Conn, next: Next) -> Conn;
}

/// Type-erased boxed plug.
pub type BoxPlug = Arc<dyn Plug>;

/// Blanket impl: any `async fn(Conn, Next) -> Conn` is a `Plug`.
#[async_trait::async_trait]
impl<F, Fut> Plug for F
where
    F: Fn(Conn, Next) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Conn> + Send + 'static,
{
    async fn call(&self, conn: Conn, next: Next) -> Conn {
        self(conn, next).await
    }
}

/// Build a composed plug chain from a list of plugs.
///
/// The plugs are wrapped in `Arc<[_]>` so the slice is shared across the
/// recursive chain without cloning the entire `Vec` at every level.
pub fn compose(plugs: Vec<BoxPlug>) -> impl Fn(Conn) -> BoxFuture<Conn> + Clone + Send + Sync {
    let plugs: Arc<[BoxPlug]> = plugs.into();
    move |conn: Conn| {
        let plugs = Arc::clone(&plugs);
        Box::pin(run_chain(conn, plugs, 0)) as BoxFuture<Conn>
    }
}

fn run_chain(
    conn: Conn,
    plugs: Arc<[BoxPlug]>,
    idx: usize,
) -> Pin<Box<dyn Future<Output = Conn> + Send>> {
    Box::pin(async move {
        if conn.halted || idx >= plugs.len() {
            return conn;
        }
        let plug = Arc::clone(&plugs[idx]);
        let next = Next::new(move |c: Conn| {
            let plugs = Arc::clone(&plugs);
            Box::pin(run_chain(c, plugs, idx + 1)) as BoxFuture<Conn>
        });
        plug.call(conn, next).await
    })
}
