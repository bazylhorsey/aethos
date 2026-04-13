//! `Query` — composable query builder.
//!
//! Builds parameterised SQL strings for use with `sqlx::query`.
//!
//! # Example
//!
//! ```rust
//! use aethos_orm::Query;
//!
//! let q = Query::from("users")
//!     .select(&["id", "name", "email"])
//!     .where_clause("active = true")
//!     .order_by("name", "ASC")
//!     .limit(20)
//!     .offset(0);
//!
//! assert!(q.to_sql().contains("WHERE active = true"));
//! assert!(q.to_sql().contains("LIMIT 20"));
//! ```

/// Composable query builder. Produces a SQL string; bind values are
/// managed by the caller using sqlx's `query()` API.
#[derive(Debug, Clone, Default)]
pub struct Query {
    table:   String,
    selects: Vec<String>,
    wheres:  Vec<String>,
    orders:  Vec<String>,
    limit:   Option<u64>,
    offset:  Option<u64>,
}

impl Query {
    pub fn from(table: &str) -> Self {
        Self { table: table.to_owned(), ..Default::default() }
    }

    pub fn select(mut self, cols: &[&str]) -> Self {
        self.selects = cols.iter().map(|s| s.to_string()).collect();
        self
    }

    pub fn where_clause(mut self, condition: &str) -> Self {
        self.wheres.push(condition.to_owned());
        self
    }

    pub fn and_where(self, condition: &str) -> Self { self.where_clause(condition) }

    pub fn order_by(mut self, col: &str, dir: &str) -> Self {
        self.orders.push(format!("{col} {dir}"));
        self
    }

    pub fn limit(mut self, n: u64) -> Self { self.limit = Some(n); self }
    pub fn offset(mut self, n: u64) -> Self { self.offset = Some(n); self }

    /// Render to a SQL string. Bind parameters must be supplied separately.
    pub fn to_sql(&self) -> String {
        let cols = if self.selects.is_empty() {
            "*".to_owned()
        } else {
            self.selects.join(", ")
        };
        let mut sql = format!("SELECT {cols} FROM {}", self.table);

        if !self.wheres.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&self.wheres.join(" AND "));
        }
        if !self.orders.is_empty() {
            sql.push_str(" ORDER BY ");
            sql.push_str(&self.orders.join(", "));
        }
        if let Some(l) = self.limit  { sql.push_str(&format!(" LIMIT {l}")); }
        if let Some(o) = self.offset { sql.push_str(&format!(" OFFSET {o}")); }
        sql
    }
}

impl std::fmt::Display for Query {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_sql())
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_select_all() {
        let sql = Query::from("users").to_sql();
        assert_eq!(sql, "SELECT * FROM users");
    }

    #[test]
    fn select_columns() {
        let sql = Query::from("users").select(&["id", "name"]).to_sql();
        assert_eq!(sql, "SELECT id, name FROM users");
    }

    #[test]
    fn where_and_order() {
        let sql = Query::from("posts")
            .where_clause("published = true")
            .order_by("created_at", "DESC")
            .limit(10)
            .to_sql();
        assert_eq!(sql, "SELECT * FROM posts WHERE published = true ORDER BY created_at DESC LIMIT 10");
    }

    #[test]
    fn multiple_where_clauses() {
        let sql = Query::from("users")
            .where_clause("active = true")
            .and_where("age > 18")
            .to_sql();
        assert!(sql.contains("active = true AND age > 18"));
    }
}
