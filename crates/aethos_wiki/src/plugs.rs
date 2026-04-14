use aethos::{Conn, Next, Plug};
use aethos::async_trait;
use crate::state::AppState;

/// Reads `AppState` from axum extensions (set by `.layer(Extension(state))`)
/// and places it into `conn.assigns` for controller access via `conn.get_assign::<AppState>()`.
#[derive(Default)]
pub struct FetchState;

#[async_trait]
impl Plug for FetchState {
    async fn call(&self, mut conn: Conn, next: Next) -> Conn {
        if let Some(state) = conn.request.extensions().get::<AppState>().cloned() {
            conn = conn.assign(state);
        }
        next.run(conn).await
    }
}
