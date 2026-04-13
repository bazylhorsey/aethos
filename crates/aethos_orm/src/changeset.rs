//! `Changeset` — validates and casts user input before database operations.
//!
//! Mirrors the cast → validate → apply pipeline from Ecto, but in pure Rust.
//!
//! # Example
//!
//! ```rust
//! use aethos_orm::Changeset;
//!
//! let cs = Changeset::new()
//!     .cast_str("name", Some("Alice"))
//!     .cast_str("email", Some("alice@example.com"))
//!     .validate_required("name")
//!     .validate_required("email")
//!     .validate_length("name", 2, 100);
//!
//! assert!(cs.is_valid());
//! assert_eq!(cs.get("name"), Some("Alice"));
//! ```

use std::collections::HashMap;

/// A single validation failure.
#[derive(Debug, Clone)]
pub struct ChangesetError {
    pub field: String,
    pub message: String,
}

/// Accumulated field values + validation errors.
#[derive(Debug, Default)]
pub struct Changeset {
    data:   HashMap<String, String>,
    errors: Vec<ChangesetError>,
}

impl Changeset {
    pub fn new() -> Self { Self::default() }

    // ── Casting ───────────────────────────────────────────────────────────

    /// Accept a raw string value for `field`. `None` stores an empty string.
    pub fn cast_str(mut self, field: &str, value: Option<&str>) -> Self {
        self.data.insert(field.to_owned(), value.unwrap_or("").to_owned());
        self
    }

    /// Accept a pre-serialised JSON value.
    pub fn cast_json(mut self, field: &str, value: serde_json::Value) -> Self {
        self.data.insert(field.to_owned(), value.to_string());
        self
    }

    // ── Validations ───────────────────────────────────────────────────────

    /// Field must be present and non-empty.
    pub fn validate_required(mut self, field: &str) -> Self {
        if self.data.get(field).map(|v| v.is_empty()).unwrap_or(true) {
            self.errors.push(ChangesetError {
                field: field.to_owned(),
                message: "can't be blank".to_owned(),
            });
        }
        self
    }

    /// Field value length must be within `[min, max]`.
    pub fn validate_length(mut self, field: &str, min: usize, max: usize) -> Self {
        if let Some(v) = self.data.get(field) {
            let len = v.chars().count();
            if len < min || len > max {
                self.errors.push(ChangesetError {
                    field: field.to_owned(),
                    message: format!("must be between {min} and {max} characters"),
                });
            }
        }
        self
    }

    /// Field value must match `pattern`.
    pub fn validate_format(mut self, field: &str, pattern: &str) -> Self {
        if let Some(v) = self.data.get(field) {
            // Use a simple contains check; callers can provide full regex via validate_with
            if !v.contains(pattern) {
                self.errors.push(ChangesetError {
                    field: field.to_owned(),
                    message: format!("has invalid format (expected to contain `{pattern}`)"),
                });
            }
        }
        self
    }

    /// Custom validator: `f(value) -> Option<error_message>`.
    pub fn validate_with<F>(mut self, field: &str, f: F) -> Self
    where
        F: Fn(&str) -> Option<String>,
    {
        if let Some(v) = self.data.get(field) {
            if let Some(msg) = f(v) {
                self.errors.push(ChangesetError { field: field.to_owned(), message: msg });
            }
        }
        self
    }

    // ── Inspection ────────────────────────────────────────────────────────

    pub fn is_valid(&self) -> bool { self.errors.is_empty() }

    pub fn errors(&self) -> &[ChangesetError] { &self.errors }

    pub fn errors_for(&self, field: &str) -> Vec<&ChangesetError> {
        self.errors.iter().filter(|e| e.field == field).collect()
    }

    /// Retrieve a cast field value.
    pub fn get(&self, field: &str) -> Option<&str> {
        self.data.get(field).map(String::as_str)
    }

    /// Consume the changeset and return all field values, or an error listing
    /// the accumulated validation failures.
    pub fn apply(self) -> Result<HashMap<String, String>, Vec<ChangesetError>> {
        if self.errors.is_empty() { Ok(self.data) } else { Err(self.errors) }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_changeset() {
        let cs = Changeset::new()
            .cast_str("name", Some("Alice"))
            .cast_str("email", Some("alice@example.com"))
            .validate_required("name")
            .validate_required("email");
        assert!(cs.is_valid());
        assert_eq!(cs.get("name"), Some("Alice"));
    }

    #[test]
    fn missing_required_field() {
        let cs = Changeset::new()
            .cast_str("name", None)
            .validate_required("name");
        assert!(!cs.is_valid());
        assert_eq!(cs.errors_for("name").len(), 1);
    }

    #[test]
    fn length_validation() {
        let cs = Changeset::new()
            .cast_str("bio", Some("Hi"))
            .validate_length("bio", 10, 500);
        assert!(!cs.is_valid());
    }

    #[test]
    fn apply_returns_data_on_success() {
        let data = Changeset::new()
            .cast_str("name", Some("Bob"))
            .apply()
            .unwrap();
        assert_eq!(data["name"], "Bob");
    }

    #[test]
    fn apply_returns_errors_on_failure() {
        let errs = Changeset::new()
            .cast_str("name", None)
            .validate_required("name")
            .apply()
            .unwrap_err();
        assert_eq!(errs.len(), 1);
    }
}
