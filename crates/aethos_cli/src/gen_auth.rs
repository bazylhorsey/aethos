use std::fs;
use std::path::Path;

pub fn cmd_gen_auth(root: &str, hasher: &str) {
    let src_auth = Path::new(root).join("src/auth");
    let migrations = Path::new(root).join("migrations");

    fs::create_dir_all(&src_auth).expect("failed to create src/auth");
    fs::create_dir_all(&migrations).expect("failed to create migrations");

    write(&src_auth.join("mod.rs"),         MOD);
    write(&src_auth.join("user.rs"),        USER);
    write(&src_auth.join("user_token.rs"),  USER_TOKEN);
    write(&src_auth.join("accounts.rs"),    accounts_template(hasher));
    write(&src_auth.join("scope.rs"),       SCOPE);
    write(&src_auth.join("plugs.rs"),       PLUGS);

    let ts = timestamp();
    write(&migrations.join(format!("{}_create_users.sql", ts)),        MIGRATION_USERS);
    write(&migrations.join(format!("{}_create_user_tokens.sql", ts)),  MIGRATION_TOKENS);

    println!("✓  src/auth/mod.rs");
    println!("✓  src/auth/user.rs");
    println!("✓  src/auth/user_token.rs");
    println!("✓  src/auth/accounts.rs");
    println!("✓  src/auth/scope.rs");
    println!("✓  src/auth/plugs.rs");
    println!("✓  migrations/{}_create_users.sql", ts);
    println!("✓  migrations/{}_create_user_tokens.sql", ts);

    let feature = match hasher { "bcrypt" => "bcrypt", _ => "argon2" };
    println!("\nNext steps:\n");
    println!("  1. Add to Cargo.toml:");
    println!("       aethos_auth = {{ version = \"0.1\", features = [\"{feature}\"] }}");
    println!();
    println!("  2. Add to your AppState:");
    println!("       pub hasher: std::sync::Arc<dyn aethos_auth::PasswordHasher>,");
    println!();
    println!("  3. Initialise in main():");
    match hasher {
        "bcrypt" => println!("       let hasher = std::sync::Arc::new(aethos_auth::BcryptHasher::default());"),
        _        => println!("       let hasher = std::sync::Arc::new(aethos_auth::Argon2Hasher::default());"),
    }
    println!();
    println!("  4. Run migrations:");
    println!("       cargo aethos db.migrate");
    println!();
    println!("  5. Add to your router:");
    println!("       get!(\"/users/register\", UserAuthController, register_form);");
    println!("       post!(\"/users/register\", UserAuthController, register);");
    println!("       get!(\"/users/login\",    UserAuthController, login_form);");
    println!("       post!(\"/users/login\",    UserAuthController, login);");
    println!("       delete!(\"/users/logout\", UserAuthController, logout);");
}

fn write(path: &Path, content: impl AsRef<str>) {
    if path.exists() {
        println!("  (skipped — already exists) {}", path.display());
        return;
    }
    fs::write(path, content.as_ref()).unwrap_or_else(|e| eprintln!("error writing {}: {e}", path.display()));
}

fn timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    // YYYYMMDDHHMMSS approximation from unix timestamp
    let s = secs;
    let sec  = s % 60;        let s = s / 60;
    let min  = s % 60;        let s = s / 60;
    let hour = s % 24;        let s = s / 24;
    // days since epoch → rough date (good enough for migration ordering)
    let days = s;
    let year = 1970 + days / 365;
    let day_of_year = days % 365;
    let month = (day_of_year / 30).min(11) + 1;
    let day   = (day_of_year % 30) + 1;
    format!("{year:04}{month:02}{day:02}{hour:02}{min:02}{sec:02}")
}

fn accounts_template(hasher: &str) -> String {
    let hasher_type = match hasher {
        "bcrypt" => "aethos_auth::BcryptHasher",
        _        => "aethos_auth::Argon2Hasher",
    };
    format!(r#"use std::sync::Arc;
use aethos::orm::{{Changeset, OrmError}};
use aethos_auth::{{PasswordHasher, generate_token, hash_token, AuthError}};
use super::user::User;
use super::user_token::UserToken;

/// Type alias — swap `{hasher_type}` for any `PasswordHasher` impl.
pub type Hasher = {hasher_type};

#[derive(Debug, thiserror::Error)]
pub enum AccountsError {{
    #[error("invalid email or password")]
    InvalidCredentials,
    #[error("email already taken")]
    EmailTaken,
    #[error(transparent)]
    Auth(#[from] AuthError),
    #[error(transparent)]
    Orm(#[from] OrmError),
    #[error(transparent)]
    Db(#[from] sqlx::Error),
}}

pub async fn register_user(
    pool:     &sqlx::SqlitePool,
    hasher:   &dyn PasswordHasher,
    email:    &str,
    password: &str,
) -> Result<User, AccountsError> {{
    let exists: Option<User> = sqlx::query_as("SELECT * FROM users WHERE email = ?1 LIMIT 1")
        .bind(email)
        .fetch_optional(pool)
        .await?;
    if exists.is_some() {{
        return Err(AccountsError::EmailTaken);
    }}
    let hashed_password = hasher.hash(password)?;
    sqlx::query("INSERT INTO users (email, hashed_password) VALUES (?1, ?2)")
        .bind(email)
        .bind(&hashed_password)
        .execute(pool)
        .await?;
    let user: User = sqlx::query_as("SELECT * FROM users WHERE email = ?1 LIMIT 1")
        .bind(email)
        .fetch_one(pool)
        .await?;
    Ok(user)
}}

pub async fn authenticate_user(
    pool:     &sqlx::SqlitePool,
    hasher:   &dyn PasswordHasher,
    email:    &str,
    password: &str,
) -> Result<User, AccountsError> {{
    let user: Option<User> = sqlx::query_as("SELECT * FROM users WHERE email = ?1 LIMIT 1")
        .bind(email)
        .fetch_optional(pool)
        .await?;
    match user {{
        Some(u) if hasher.verify(password, &u.hashed_password)? => Ok(u),
        _ => Err(AccountsError::InvalidCredentials),
    }}
}}

pub async fn create_session_token(pool: &sqlx::SqlitePool, user: &User) -> Result<String, AccountsError> {{
    let (raw, hashed) = generate_token();
    sqlx::query(
        "INSERT INTO user_tokens (user_id, token, context, sent_to) VALUES (?1, ?2, 'session', ?3)"
    )
    .bind(user.id)
    .bind(&hashed)
    .bind(&user.email)
    .execute(pool)
    .await?;
    Ok(raw)
}}

pub async fn get_user_by_session_token(pool: &sqlx::SqlitePool, raw: &str) -> Option<User> {{
    let hashed = hash_token(raw);
    sqlx::query_as(
        "SELECT u.* FROM users u
         JOIN user_tokens t ON t.user_id = u.id
         WHERE t.token = ?1 AND t.context = 'session'"
    )
    .bind(&hashed)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
}}

pub async fn delete_session_token(pool: &sqlx::SqlitePool, raw: &str) -> Result<(), AccountsError> {{
    let hashed = hash_token(raw);
    sqlx::query("DELETE FROM user_tokens WHERE token = ?1")
        .bind(&hashed)
        .execute(pool)
        .await?;
    Ok(())
}}

pub async fn create_api_token(pool: &sqlx::SqlitePool, user: &User) -> Result<String, AccountsError> {{
    let (raw, hashed) = generate_token();
    sqlx::query(
        "INSERT INTO user_tokens (user_id, token, context, sent_to) VALUES (?1, ?2, 'api', ?3)"
    )
    .bind(user.id)
    .bind(&hashed)
    .bind(&user.email)
    .execute(pool)
    .await?;
    Ok(raw)
}}

pub async fn get_user_by_api_token(pool: &sqlx::SqlitePool, raw: &str) -> Option<User> {{
    let hashed = hash_token(raw);
    sqlx::query_as(
        "SELECT u.* FROM users u
         JOIN user_tokens t ON t.user_id = u.id
         WHERE t.token = ?1 AND t.context = 'api'"
    )
    .bind(&hashed)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
}}

pub fn registration_changeset(email: Option<&str>, password: Option<&str>) -> Changeset {{
    Changeset::new()
        .cast_str("email",    email)
        .cast_str("password", password)
        .validate_required("email")
        .validate_length("email", 3, 160)
        .validate_required("password")
        .validate_length("password", 12, 72)
}}
"#)
}

const MOD: &str = r#"pub mod user;
pub mod user_token;
pub mod accounts;
pub mod scope;
pub mod plugs;
"#;

const USER: &str = r#"use aethos::orm::Schema;

#[derive(Debug, Clone, sqlx::FromRow, Schema)]
#[schema(table = "users")]
pub struct User {
    #[field(primary_key)]
    pub id:              i64,
    pub email:           String,
    pub hashed_password: String,
    pub confirmed_at:    Option<String>,
}
"#;

const USER_TOKEN: &str = r#"use aethos::orm::Schema;

#[derive(Debug, Clone, sqlx::FromRow, Schema)]
#[schema(table = "user_tokens")]
pub struct UserToken {
    #[field(primary_key)]
    pub id:          i64,
    pub user_id:     i64,
    pub token:       String,
    pub context:     String,
    pub sent_to:     Option<String>,
    pub inserted_at: String,
}
"#;

const SCOPE: &str = r#"use super::user::User;

/// Scope carries the authenticated user (and any future context: team, org, roles).
/// Store it in conn assigns as `current_scope` — mirrors Phoenix's Scope pattern.
#[derive(Debug, Clone)]
pub struct Scope {
    pub user: User,
}

impl Scope {
    pub fn for_user(user: User) -> Self {
        Self { user }
    }
}
"#;

const PLUGS: &str = r#"use aethos::{Conn, Next, Plug};
use aethos::async_trait;
use super::accounts::get_user_by_session_token;
use super::scope::Scope;

/// Reads the session token, resolves the current user, and stores a `Scope`
/// in conn assigns as `current_scope`.  Equivalent to Phoenix's
/// `fetch_current_scope_for_user/2` plug.
#[derive(Default)]
pub struct FetchCurrentUser;

#[async_trait]
impl Plug for FetchCurrentUser {
    async fn call(&self, mut conn: Conn, next: Next) -> Conn {
        // TODO: replace with your AppState's pool reference
        // let pool = conn.get_assign::<AppState>().map(|s| s.pool.clone());
        // if let Some((pool, token)) = pool.zip(conn.session.get::<String>("user_token")) {
        //     if let Some(user) = get_user_by_session_token(&pool, &token).await {
        //         conn = conn.assign(Scope::for_user(user));
        //     }
        // }
        next.run(conn).await
    }
}

/// Requires an authenticated user.  Redirects to `/users/login` otherwise.
/// Equivalent to Phoenix's `require_authenticated_user/2` plug.
#[derive(Default)]
pub struct RequireAuthenticated;

#[async_trait]
impl Plug for RequireAuthenticated {
    async fn call(&self, conn: Conn, next: Next) -> Conn {
        if conn.get_assign::<Scope>().is_none() {
            return conn.redirect("/users/login");
        }
        next.run(conn).await
    }
}

/// Redirects already-authenticated users away from login/register pages.
/// Equivalent to Phoenix's `redirect_if_user_is_authenticated/2` plug.
#[derive(Default)]
pub struct RedirectIfAuthenticated;

#[async_trait]
impl Plug for RedirectIfAuthenticated {
    async fn call(&self, conn: Conn, next: Next) -> Conn {
        if conn.get_assign::<Scope>().is_some() {
            return conn.redirect("/");
        }
        next.run(conn).await
    }
}
"#;

const MIGRATION_USERS: &str = r#"-- +migrate Up
CREATE TABLE IF NOT EXISTS users (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    email           TEXT    NOT NULL UNIQUE,
    hashed_password TEXT    NOT NULL,
    confirmed_at    TEXT,
    inserted_at     TEXT    NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT    NOT NULL DEFAULT (datetime('now'))
);

-- +migrate Down
DROP TABLE IF EXISTS users;
"#;

const MIGRATION_TOKENS: &str = r#"-- +migrate Up
CREATE TABLE IF NOT EXISTS user_tokens (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id     INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token       TEXT    NOT NULL UNIQUE,
    context     TEXT    NOT NULL,
    sent_to     TEXT,
    inserted_at TEXT    NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_user_tokens_context ON user_tokens(context);
CREATE INDEX idx_user_tokens_user_id ON user_tokens(user_id);

-- +migrate Down
DROP TABLE IF EXISTS user_tokens;
"#;
