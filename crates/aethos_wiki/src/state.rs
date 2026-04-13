use std::sync::Arc;
use aethos_orm::Repo;

#[derive(Clone)]
pub struct AppState {
    pub repo: Arc<Repo<sqlx::Sqlite>>,
}
