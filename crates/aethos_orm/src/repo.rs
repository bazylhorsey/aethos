//! `Repo` — connection pool + CRUD operations.
//!
//! Wraps [`sqlx::Pool`] and provides typed helpers that work with any struct
//! implementing [`Schema`](crate::schema::Schema).
//!
//! # Example
//!
//! ```rust,ignore
//! use aethos_orm::{Repo, AnyPool};
//!
//! let pool = sqlx::postgres::PgPoolOptions::new()
//!     .max_connections(10)
//!     .connect("postgres://localhost/myapp_dev")
//!     .await?;
//!
//! let repo = Repo::new(pool);
//!
//! // Insert
//! let user: User = repo.insert("users", &params).await?;
//!
//! // Fetch one
//! let user: User = repo.get("users", 1_i64).await?;
//!
//! // Fetch all
//! let users: Vec<User> = repo.all("users").await?;
//!
//! // Delete
//! repo.delete("users", 1_i64).await?;
//! ```

use std::sync::Arc;

use serde::{de::DeserializeOwned, Serialize};
use sqlx::{Database, Pool};

use crate::error::OrmError;

/// Type alias for a pool that erases the concrete DB type (SQLite or Postgres).
pub type AnyPool = sqlx::AnyPool;

/// Connection-pool wrapper. `Db` is the sqlx database driver (e.g. `sqlx::Sqlite`).
#[derive(Clone)]
pub struct Repo<Db: Database> {
    pool: Arc<Pool<Db>>,
}

impl<Db: Database> Repo<Db> {
    pub fn new(pool: Pool<Db>) -> Self {
        Self { pool: Arc::new(pool) }
    }

    pub fn pool(&self) -> &Pool<Db> { &self.pool }
}

// ── Sqlite convenience impl ───────────────────────────────────────────────────

impl Repo<sqlx::Sqlite> {
    pub async fn connect(url: &str) -> Result<Self, OrmError> {
        let pool = sqlx::SqlitePool::connect(url).await?;
        Ok(Self::new(pool))
    }

    /// Run a raw SQL string and return affected rows.
    pub async fn execute(&self, sql: &str) -> Result<u64, OrmError> {
        let r = sqlx::query(sql).execute(self.pool()).await?;
        Ok(r.rows_affected())
    }

    /// Insert a JSON-serializable record. Returns the auto-generated integer id.
    pub async fn insert_json<T: Serialize>(
        &self, table: &str, record: &T,
    ) -> Result<i64, OrmError> {
        let json = serde_json::to_value(record)
            .map_err(|e| OrmError::Validation(e.to_string()))?;
        let obj = json.as_object()
            .ok_or_else(|| OrmError::Validation("record must be a JSON object".into()))?;

        let cols: Vec<&str> = obj.keys().map(String::as_str).collect();
        let placeholders: Vec<String> = (1..=cols.len()).map(|i| format!("?{i}")).collect();
        let sql = format!(
            "INSERT INTO {} ({}) VALUES ({})",
            table,
            cols.join(", "),
            placeholders.join(", ")
        );

        let mut q = sqlx::query(&sql);
        for col in &cols {
            match &obj[*col] {
                serde_json::Value::Null         => q = q.bind(None::<String>),
                serde_json::Value::Bool(b)      => q = q.bind(*b),
                serde_json::Value::Number(n)    => {
                    if let Some(i) = n.as_i64() { q = q.bind(i); }
                    else { q = q.bind(n.as_f64().unwrap_or(0.0)); }
                }
                serde_json::Value::String(s)    => q = q.bind(s.clone()),
                other                           => q = q.bind(other.to_string()),
            }
        }
        let result = q.execute(self.pool()).await?;
        Ok(result.last_insert_rowid())
    }

    /// Fetch all rows from `table` as the deserialized type `T`.
    pub async fn all<T>(&self, table: &str) -> Result<Vec<T>, OrmError>
    where
        T: DeserializeOwned + for<'r> sqlx::FromRow<'r, sqlx::sqlite::SqliteRow> + Send + Unpin,
    {
        let sql = format!("SELECT * FROM {table}");
        let rows = sqlx::query_as::<_, T>(&sql)
            .fetch_all(self.pool())
            .await?;
        Ok(rows)
    }

    /// Fetch a single row by integer primary key from `id_col` (default `id`).
    pub async fn get<T>(&self, table: &str, id: i64) -> Result<T, OrmError>
    where
        T: DeserializeOwned + for<'r> sqlx::FromRow<'r, sqlx::sqlite::SqliteRow> + Send + Unpin,
    {
        let sql = format!("SELECT * FROM {table} WHERE id = ?1 LIMIT 1");
        sqlx::query_as::<_, T>(&sql)
            .bind(id)
            .fetch_optional(self.pool())
            .await?
            .ok_or(OrmError::NotFound)
    }

    /// Delete a row by integer primary key.
    pub async fn delete(&self, table: &str, id: i64) -> Result<(), OrmError> {
        let sql = format!("DELETE FROM {table} WHERE id = ?1");
        sqlx::query(&sql).bind(id).execute(self.pool()).await?;
        Ok(())
    }
}

// ── Postgres convenience impl ─────────────────────────────────────────────────

impl Repo<sqlx::Postgres> {
    pub async fn connect(url: &str) -> Result<Self, OrmError> {
        let pool = sqlx::PgPool::connect(url).await?;
        Ok(Self::new(pool))
    }

    pub async fn execute(&self, sql: &str) -> Result<u64, OrmError> {
        let r = sqlx::query(sql).execute(self.pool()).await?;
        Ok(r.rows_affected())
    }

    pub async fn all<T>(&self, table: &str) -> Result<Vec<T>, OrmError>
    where
        T: DeserializeOwned + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin,
    {
        let sql = format!("SELECT * FROM {table}");
        Ok(sqlx::query_as::<_, T>(&sql).fetch_all(self.pool()).await?)
    }

    pub async fn get<T>(&self, table: &str, id: i64) -> Result<T, OrmError>
    where
        T: DeserializeOwned + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin,
    {
        let sql = format!("SELECT * FROM {table} WHERE id = $1 LIMIT 1");
        sqlx::query_as::<_, T>(&sql)
            .bind(id)
            .fetch_optional(self.pool())
            .await?
            .ok_or(OrmError::NotFound)
    }

    pub async fn delete(&self, table: &str, id: i64) -> Result<(), OrmError> {
        sqlx::query(&format!("DELETE FROM {table} WHERE id = $1"))
            .bind(id)
            .execute(self.pool())
            .await?;
        Ok(())
    }
}
