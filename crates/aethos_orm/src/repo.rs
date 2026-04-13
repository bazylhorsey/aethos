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

        // Single pass: build both column list and placeholder list simultaneously.
        // Avoids two intermediate Vecs and a second iteration to bind values.
        let mut cols = String::new();
        let mut placeholders = String::new();
        for (i, key) in obj.keys().enumerate() {
            if i > 0 { cols.push_str(", "); placeholders.push_str(", "); }
            cols.push_str(key);
            // push_str + itoa-style avoids format! allocation per placeholder
            placeholders.push('?');
            let mut buf = itoa::Buffer::new();
            placeholders.push_str(buf.format(i + 1));
        }
        let sql = format!("INSERT INTO {table} ({cols}) VALUES ({placeholders})");

        let mut q = sqlx::query(&sql);
        for val in obj.values() {
            q = bind_json_value(q, val);
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

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Bind a single `serde_json::Value` onto a sqlx query without re-allocating
/// the value — strings are cloned (unavoidable with sqlx's owned-bind API),
/// but numbers and booleans are bound as their native types.
fn bind_json_value<'q, DB>(
    q: sqlx::query::Query<'q, DB, DB::Arguments<'q>>,
    val: &serde_json::Value,
) -> sqlx::query::Query<'q, DB, DB::Arguments<'q>>
where
    DB: sqlx::Database,
    bool:         sqlx::Encode<'q, DB> + sqlx::Type<DB>,
    i64:          sqlx::Encode<'q, DB> + sqlx::Type<DB>,
    f64:          sqlx::Encode<'q, DB> + sqlx::Type<DB>,
    String:       sqlx::Encode<'q, DB> + sqlx::Type<DB>,
    Option<String>: sqlx::Encode<'q, DB> + sqlx::Type<DB>,
{
    match val {
        serde_json::Value::Null         => q.bind(None::<String>),
        serde_json::Value::Bool(b)      => q.bind(*b),
        serde_json::Value::Number(n)    => {
            if let Some(i) = n.as_i64() { q.bind(i) }
            else { q.bind(n.as_f64().unwrap_or(0.0)) }
        }
        serde_json::Value::String(s)    => q.bind(s.clone()),
        other                           => q.bind(other.to_string()),
    }
}
