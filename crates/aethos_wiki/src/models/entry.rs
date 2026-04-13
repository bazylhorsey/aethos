use aethos_orm::{Repo, OrmError};

#[derive(Debug, sqlx::FromRow, Clone)]
pub struct Entry {
    pub id: i64,
    pub title: String,
    pub content: String,
    pub created_at: String,
    pub updated_at: String,
}

impl Entry {
    pub async fn all(repo: &Repo<sqlx::Sqlite>) -> Vec<Self> {
        sqlx::query_as::<_, Self>("SELECT * FROM entries ORDER BY title ASC")
            .fetch_all(repo.pool())
            .await
            .unwrap_or_default()
    }

    pub async fn find_by_title(repo: &Repo<sqlx::Sqlite>, title: &str) -> Option<Self> {
        sqlx::query_as::<_, Self>(
            "SELECT * FROM entries WHERE title = ?1 LIMIT 1",
        )
        .bind(title)
        .fetch_optional(repo.pool())
        .await
        .ok()
        .flatten()
    }

    pub async fn search(repo: &Repo<sqlx::Sqlite>, query: &str) -> Vec<Self> {
        let pattern = format!("%{}%", query);
        sqlx::query_as::<_, Self>(
            "SELECT * FROM entries WHERE title LIKE ?1 ORDER BY title ASC",
        )
        .bind(pattern)
        .fetch_all(repo.pool())
        .await
        .unwrap_or_default()
    }

    pub async fn create(
        repo: &Repo<sqlx::Sqlite>,
        title: &str,
        content: &str,
    ) -> Result<(), OrmError> {
        sqlx::query(
            "INSERT INTO entries (title, content, created_at, updated_at) \
             VALUES (?1, ?2, datetime('now'), datetime('now'))",
        )
        .bind(title)
        .bind(content)
        .execute(repo.pool())
        .await?;
        Ok(())
    }

    pub async fn update(
        repo: &Repo<sqlx::Sqlite>,
        title: &str,
        content: &str,
    ) -> Result<(), OrmError> {
        sqlx::query(
            "UPDATE entries SET content = ?1, updated_at = datetime('now') WHERE title = ?2",
        )
        .bind(content)
        .bind(title)
        .execute(repo.pool())
        .await?;
        Ok(())
    }

    pub async fn count(repo: &Repo<sqlx::Sqlite>) -> i64 {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM entries")
            .fetch_one(repo.pool())
            .await
            .unwrap_or((0,));
        row.0
    }
}
