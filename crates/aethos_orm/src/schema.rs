//! `Schema` — marker trait for structs that map to a database table.
//!
//! Structs deriving `Schema` can be used directly with `Repo` methods
//! and the `Query` builder. The derive macro is in `aethos_orm_macros`
//! (planned). For now, implement it manually or use the blanket impl.
//!
//! # Example
//!
//! ```rust,ignore
//! use aethos_orm::Schema;
//! use sqlx::FromRow;
//!
//! #[derive(Debug, Schema, FromRow, serde::Serialize, serde::Deserialize)]
//! #[schema(table = "users")]
//! pub struct User {
//!     pub id:    i64,
//!     pub name:  String,
//!     pub email: String,
//! }
//! ```

/// Marker trait for structs that represent a database table.
///
/// Implement this trait (or derive it via `#[derive(Schema)]`) to unlock
/// integration with `Repo` and `Query`.
pub trait Schema {
    /// The database table name this schema maps to.
    fn table_name() -> &'static str;

    /// The primary key column name (default: `"id"`).
    fn primary_key() -> &'static str { "id" }

    /// All column names in insertion order.
    fn columns() -> &'static [&'static str];
}
