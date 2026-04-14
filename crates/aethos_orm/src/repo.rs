//! `Repo` — connection pool + CRUD operations.
//!
//! Wraps [`sqlx::Pool`] and provides typed helpers that work with any struct
//! implementing [`Schema`](crate::schema::Schema).
//!
//! # Example
//!
//! ```rust,ignore
//! use aethos_orm::{Repo, PoolConfig};
//!
//! // Simple connect
//! let repo = Repo::<sqlx::Sqlite>::connect("sqlite://dev.db").await?;
//!
//! // Configured pool (mirrors Phoenix's pool_size / timeout config)
//! let repo = Repo::<sqlx::Sqlite>::connect_with(
//!     "sqlite://dev.db",
//!     PoolConfig::default().max_connections(20).timeout_secs(30),
//! ).await?;
//!
//! // Typed insert via Schema trait — no JSON intermediate
//! let id = repo.insert(&user).await?;
//!
//! // Fetch one / all / delete
//! let user: User = repo.get("users", 1_i64).await?;
//! let users: Vec<User> = repo.all("users").await?;
//! repo.delete("users", 1_i64).await?;
//! ```

use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use sqlx::{Database, Pool};

use crate::error::OrmError;
use crate::schema::Schema;

/// Type alias for a pool that erases the concrete DB type.
pub type AnyPool = sqlx::AnyPool;

// ── PoolConfig ────────────────────────────────────────────────────────────────

/// Connection pool configuration.
///
/// Mirrors Phoenix's `config :myapp, MyApp.Repo, pool_size: 10, timeout: 15_000`.
///
/// ```rust
/// use aethos_orm::PoolConfig;
/// use std::time::Duration;
///
/// let cfg = PoolConfig::default()
///     .max_connections(20)
///     .min_connections(2)
///     .timeout_secs(30)
///     .idle_timeout(Duration::from_secs(600));
/// ```
#[derive(Debug, Clone)]
pub struct PoolConfig {
    pub max_connections: u32,
    pub min_connections: u32,
    pub connect_timeout: Duration,
    pub idle_timeout: Option<Duration>,
    pub max_lifetime: Option<Duration>,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_connections: 10,
            min_connections: 0,
            connect_timeout: Duration::from_secs(30),
            idle_timeout: Some(Duration::from_secs(600)),
            max_lifetime: Some(Duration::from_secs(1800)),
        }
    }
}

impl PoolConfig {
    pub fn max_connections(mut self, n: u32) -> Self { self.max_connections = n; self }
    pub fn min_connections(mut self, n: u32) -> Self { self.min_connections = n; self }
    pub fn timeout_secs(mut self, s: u64) -> Self {
        self.connect_timeout = Duration::from_secs(s); self
    }
    pub fn idle_timeout(mut self, d: Duration) -> Self { self.idle_timeout = Some(d); self }
    pub fn max_lifetime(mut self, d: Duration) -> Self { self.max_lifetime = Some(d); self }
}

// ── Repo ──────────────────────────────────────────────────────────────────────

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

// ── Sqlite impl ───────────────────────────────────────────────────────────────

impl Repo<sqlx::Sqlite> {
    /// Connect with default pool settings.
    pub async fn connect(url: &str) -> Result<Self, OrmError> {
        Self::connect_with(url, PoolConfig::default()).await
    }

    /// Connect with explicit pool configuration.
    pub async fn connect_with(url: &str, cfg: PoolConfig) -> Result<Self, OrmError> {
        let mut opts = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(cfg.max_connections)
            .min_connections(cfg.min_connections)
            .acquire_timeout(cfg.connect_timeout);
        if let Some(d) = cfg.idle_timeout  { opts = opts.idle_timeout(d); }
        if let Some(d) = cfg.max_lifetime  { opts = opts.max_lifetime(d); }
        let pool = opts.connect(url).await?;
        Ok(Self::new(pool))
    }

    /// Run a raw SQL string and return affected rows.
    pub async fn execute(&self, sql: &str) -> Result<u64, OrmError> {
        let r = sqlx::query(sql).execute(self.pool()).await?;
        Ok(r.rows_affected())
    }

    /// Typed insert using the [`Schema`] trait — **zero JSON intermediate**.
    ///
    /// Columns come from `T::columns()` (compile-time constant), values come
    /// from `T::to_row_values()` which calls `From` on each field directly.
    /// No `serde_json::to_value`, no HashMap key inspection.
    pub async fn insert<T>(&self, record: &T) -> Result<i64, OrmError>
    where
        T: Schema,
    {
        let cols = T::columns();
        let sql  = build_insert_sql(T::table_name(), cols, "?");
        let mut q = sqlx::query(&sql);
        for val in record.to_row_values() {
            q = val.bind_sqlite(q);
        }
        Ok(q.execute(self.pool()).await?.last_insert_rowid())
    }

    /// Dynamic insert from any serializable value (table name explicit).
    pub async fn insert_json<T: Serialize>(
        &self, table: &str, record: &T,
    ) -> Result<i64, OrmError> {
        let json = serde_json::to_value(record)
            .map_err(|e| OrmError::Validation(e.to_string()))?;
        let obj = json.as_object()
            .ok_or_else(|| OrmError::Validation("record must be a JSON object".into()))?;
        let mut cols = String::new();
        let mut placeholders = String::new();
        for (i, key) in obj.keys().enumerate() {
            if i > 0 { cols.push_str(", "); placeholders.push_str(", "); }
            cols.push_str(key);
            placeholders.push('?');
            let mut buf = itoa::Buffer::new();
            placeholders.push_str(buf.format(i + 1));
        }
        let sql = format!("INSERT INTO {table} ({cols}) VALUES ({placeholders})");
        let mut q = sqlx::query(&sql);
        for val in obj.values() { q = bind_json_value(q, val); }
        Ok(q.execute(self.pool()).await?.last_insert_rowid())
    }

    pub async fn all<T>(&self, table: &str) -> Result<Vec<T>, OrmError>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::sqlite::SqliteRow> + Send + Unpin,
    {
        Ok(sqlx::query_as::<_, T>(&format!("SELECT * FROM {table}"))
            .fetch_all(self.pool()).await?)
    }

    pub async fn get<T>(&self, table: &str, id: i64) -> Result<T, OrmError>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::sqlite::SqliteRow> + Send + Unpin,
    {
        sqlx::query_as::<_, T>(&format!("SELECT * FROM {table} WHERE id = ?1 LIMIT 1"))
            .bind(id).fetch_optional(self.pool()).await?
            .ok_or(OrmError::NotFound)
    }

    /// Typed update using the [`Schema`] trait — updates all non-PK columns by primary key.
    ///
    /// Requires `T::primary_key_value()` to return a non-Null value.
    pub async fn update<T>(&self, record: &T) -> Result<(), OrmError>
    where
        T: Schema,
    {
        let cols = T::columns();
        let set_clause: String = cols.iter().enumerate()
            .map(|(i, col)| {
                let mut s = format!("{col} = ?");
                let mut buf = itoa::Buffer::new();
                s.push_str(buf.format(i + 1));
                s
            })
            .collect::<Vec<_>>()
            .join(", ");
        let mut pk_idx_buf = itoa::Buffer::new();
        let pk_idx = pk_idx_buf.format(cols.len() + 1);
        let sql = format!(
            "UPDATE {} SET {} WHERE {} = ?{}",
            T::table_name(), set_clause, T::primary_key(), pk_idx
        );
        let mut q = sqlx::query(&sql);
        for val in record.to_row_values() {
            q = val.bind_sqlite(q);
        }
        q = record.primary_key_value().bind_sqlite(q);
        q.execute(self.pool()).await?;
        Ok(())
    }

    pub async fn delete(&self, table: &str, id: i64) -> Result<(), OrmError> {
        sqlx::query(&format!("DELETE FROM {table} WHERE id = ?1"))
            .bind(id).execute(self.pool()).await?;
        Ok(())
    }
}

// ── Postgres impl ─────────────────────────────────────────────────────────────

impl Repo<sqlx::Postgres> {
    pub async fn connect(url: &str) -> Result<Self, OrmError> {
        Self::connect_with(url, PoolConfig::default()).await
    }

    pub async fn connect_with(url: &str, cfg: PoolConfig) -> Result<Self, OrmError> {
        let mut opts = sqlx::postgres::PgPoolOptions::new()
            .max_connections(cfg.max_connections)
            .min_connections(cfg.min_connections)
            .acquire_timeout(cfg.connect_timeout);
        if let Some(d) = cfg.idle_timeout  { opts = opts.idle_timeout(d); }
        if let Some(d) = cfg.max_lifetime  { opts = opts.max_lifetime(d); }
        let pool = opts.connect(url).await?;
        Ok(Self::new(pool))
    }

    pub async fn execute(&self, sql: &str) -> Result<u64, OrmError> {
        Ok(sqlx::query(sql).execute(self.pool()).await?.rows_affected())
    }

    /// Typed insert for Postgres — zero JSON, uses `$1, $2, …` placeholders.
    pub async fn insert<T>(&self, record: &T) -> Result<(), OrmError>
    where
        T: Schema,
    {
        let cols = T::columns();
        let sql  = build_insert_sql(T::table_name(), cols, "$");
        let mut q = sqlx::query(&sql);
        for val in record.to_row_values() {
            q = val.bind_postgres(q);
        }
        q.execute(self.pool()).await?;
        Ok(())
    }

    pub async fn all<T>(&self, table: &str) -> Result<Vec<T>, OrmError>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin,
    {
        Ok(sqlx::query_as::<_, T>(&format!("SELECT * FROM {table}"))
            .fetch_all(self.pool()).await?)
    }

    pub async fn get<T>(&self, table: &str, id: i64) -> Result<T, OrmError>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin,
    {
        sqlx::query_as::<_, T>(&format!("SELECT * FROM {table} WHERE id = $1 LIMIT 1"))
            .bind(id).fetch_optional(self.pool()).await?
            .ok_or(OrmError::NotFound)
    }

    pub async fn delete(&self, table: &str, id: i64) -> Result<(), OrmError> {
        sqlx::query(&format!("DELETE FROM {table} WHERE id = $1"))
            .bind(id).execute(self.pool()).await?;
        Ok(())
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Build `INSERT INTO table (col1, col2) VALUES (?1, ?2)` from compile-time
/// column metadata. `placeholder_prefix` is `"?"` for SQLite, `"$"` for Postgres.
fn build_insert_sql(table: &str, cols: &[&str], placeholder_prefix: &str) -> String {
    let mut col_list   = String::new();
    let mut value_list = String::new();
    for (i, col) in cols.iter().enumerate() {
        if i > 0 { col_list.push_str(", "); value_list.push_str(", "); }
        col_list.push_str(col);
        value_list.push_str(placeholder_prefix);
        let mut buf = itoa::Buffer::new();
        value_list.push_str(buf.format(i + 1));
    }
    format!("INSERT INTO {table} ({col_list}) VALUES ({value_list})")
}

fn bind_json_value<'q, DB>(
    q: sqlx::query::Query<'q, DB, DB::Arguments<'q>>,
    val: &serde_json::Value,
) -> sqlx::query::Query<'q, DB, DB::Arguments<'q>>
where
    DB: sqlx::Database,
    bool:           sqlx::Encode<'q, DB> + sqlx::Type<DB>,
    i64:            sqlx::Encode<'q, DB> + sqlx::Type<DB>,
    f64:            sqlx::Encode<'q, DB> + sqlx::Type<DB>,
    String:         sqlx::Encode<'q, DB> + sqlx::Type<DB>,
    Option<String>: sqlx::Encode<'q, DB> + sqlx::Type<DB>,
{
    match val {
        serde_json::Value::Null      => q.bind(None::<String>),
        serde_json::Value::Bool(b)   => q.bind(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() { q.bind(i) }
            else { q.bind(n.as_f64().unwrap_or(0.0)) }
        }
        serde_json::Value::String(s) => q.bind(s.clone()),
        other                        => q.bind(other.to_string()),
    }
}
