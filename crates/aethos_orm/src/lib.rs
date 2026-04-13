//! `aethos_orm` — database access layer for Aethos.
//!
//! Provides an Ecto-inspired API without any Elixir terminology:
//!
//! ```text
//! Repo       — connection pool + CRUD, wraps sqlx::Pool
//! Schema     — #[derive(Schema)] maps struct fields to table columns
//! Changeset  — validates and casts user input before touching the DB
//! Query      — composable query builder (select/where/order/limit)
//! Migration  — runs versioned SQL files from priv/repo/migrations/
//! ```

pub mod repo;
pub mod schema;
pub mod changeset;
pub mod query;
pub mod migration;
pub mod error;
pub mod value;

pub use repo::{Repo, AnyPool, PoolConfig};
pub use schema::Schema;
pub use changeset::{Changeset, ChangesetError};
pub use query::Query;
pub use migration::MigrationRunner;
pub use error::OrmError;
pub use value::SqlValue;

/// Re-export the `#[derive(Schema)]` macro.
pub use aethos_orm_macros::Schema;

pub use sqlx;
pub use chrono;
pub use uuid;
