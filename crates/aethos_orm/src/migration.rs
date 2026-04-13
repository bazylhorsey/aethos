//! `MigrationRunner` — applies versioned SQL migration files.
//!
//! Scans a directory for `*.sql` files named with a numeric prefix (e.g.
//! `20240101_create_users.sql`) and runs them in order, tracking applied
//! migrations in a `_migrations` table.
//!
//! # Example
//!
//! ```rust,ignore
//! use aethos_orm::MigrationRunner;
//!
//! let runner = MigrationRunner::new("priv/repo/migrations");
//! runner.run(&repo.pool()).await?;
//! ```

use std::path::{Path, PathBuf};

use sqlx::SqlitePool;
use tracing::{info, warn};

use crate::error::OrmError;

pub struct MigrationRunner {
    dir: PathBuf,
}

impl MigrationRunner {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// Apply all pending migrations to a SQLite pool.
    pub async fn run(&self, pool: &SqlitePool) -> Result<(), OrmError> {
        self.ensure_migrations_table(pool).await?;
        let mut files = self.collect_migration_files()?;
        files.sort();

        for path in &files {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_owned();

            if self.is_applied(pool, &name).await? {
                continue;
            }

            let sql = std::fs::read_to_string(path)
                .map_err(|e| OrmError::Migration(format!("cannot read {name}: {e}")))?;

            sqlx::query(&sql)
                .execute(pool)
                .await
                .map_err(|e| OrmError::Migration(format!("{name}: {e}")))?;

            self.record_applied(pool, &name).await?;
            info!(migration = %name, "applied");
        }
        Ok(())
    }

    /// Roll back the last applied migration.
    pub async fn rollback(&self, pool: &SqlitePool) -> Result<(), OrmError> {
        self.ensure_migrations_table(pool).await?;
        let last: Option<(String,)> = sqlx::query_as(
            "SELECT name FROM _migrations ORDER BY applied_at DESC LIMIT 1",
        )
        .fetch_optional(pool)
        .await?;

        let Some((name,)) = last else {
            warn!("no migrations to roll back");
            return Ok(());
        };

        // Convention: a matching `<name>.down.sql` is the rollback file.
        let down_path = self.dir.join(format!("{}.down.sql", name.trim_end_matches(".sql")));
        if down_path.exists() {
            let sql = std::fs::read_to_string(&down_path)
                .map_err(|e| OrmError::Migration(format!("cannot read rollback: {e}")))?;
            sqlx::query(&sql).execute(pool).await?;
        } else {
            warn!(migration = %name, "no .down.sql found, skipping rollback SQL");
        }

        sqlx::query("DELETE FROM _migrations WHERE name = ?1")
            .bind(&name)
            .execute(pool)
            .await?;
        info!(migration = %name, "rolled back");
        Ok(())
    }

    /// List all applied migrations.
    pub async fn status(&self, pool: &SqlitePool) -> Result<Vec<(String, String)>, OrmError> {
        self.ensure_migrations_table(pool).await?;
        let rows: Vec<(String, String)> =
            sqlx::query_as("SELECT name, applied_at FROM _migrations ORDER BY applied_at")
                .fetch_all(pool)
                .await?;
        Ok(rows)
    }

    async fn ensure_migrations_table(&self, pool: &SqlitePool) -> Result<(), OrmError> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS _migrations (
                name       TEXT PRIMARY KEY,
                applied_at TEXT NOT NULL DEFAULT (datetime('now'))
            )",
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    async fn is_applied(&self, pool: &SqlitePool, name: &str) -> Result<bool, OrmError> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT name FROM _migrations WHERE name = ?1")
                .bind(name)
                .fetch_optional(pool)
                .await?;
        Ok(row.is_some())
    }

    async fn record_applied(&self, pool: &SqlitePool, name: &str) -> Result<(), OrmError> {
        sqlx::query("INSERT INTO _migrations (name) VALUES (?1)")
            .bind(name)
            .execute(pool)
            .await?;
        Ok(())
    }

    fn collect_migration_files(&self) -> Result<Vec<PathBuf>, OrmError> {
        let dir = Path::new(&self.dir);
        if !dir.exists() {
            return Ok(vec![]);
        }
        let entries = std::fs::read_dir(dir)
            .map_err(|e| OrmError::Migration(format!("cannot read migrations dir: {e}")))?;
        let mut files = Vec::new();
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().and_then(|e| e.to_str()) == Some("sql")
                && !p.to_str().map(|s| s.ends_with(".down.sql")).unwrap_or(false)
            {
                files.push(p);
            }
        }
        Ok(files)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn run_migrations_from_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let mig_dir = tmp.path().join("migrations");
        std::fs::create_dir(&mig_dir).unwrap();

        std::fs::write(
            mig_dir.join("001_create_users.sql"),
            "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL);",
        )
        .unwrap();

        let pool = sqlx::SqlitePool::connect(":memory:").await.unwrap();
        MigrationRunner::new(&mig_dir).run(&pool).await.unwrap();

        // Running again is idempotent
        MigrationRunner::new(&mig_dir).run(&pool).await.unwrap();

        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM _migrations")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count.0, 1);
    }

    #[tokio::test]
    async fn rollback_last_migration() {
        let tmp = tempfile::tempdir().unwrap();
        let mig_dir = tmp.path().join("migrations");
        std::fs::create_dir(&mig_dir).unwrap();

        std::fs::write(
            mig_dir.join("001_create_tags.sql"),
            "CREATE TABLE tags (id INTEGER PRIMARY KEY, label TEXT);",
        )
        .unwrap();
        std::fs::write(
            mig_dir.join("001_create_tags.down.sql"),
            "DROP TABLE tags;",
        )
        .unwrap();

        let pool = sqlx::SqlitePool::connect(":memory:").await.unwrap();
        let runner = MigrationRunner::new(&mig_dir);
        runner.run(&pool).await.unwrap();
        runner.rollback(&pool).await.unwrap();

        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM _migrations")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count.0, 0);
    }
}
