use std::path::Path;
use std::process;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // When invoked as `cargo aethos`, the first arg is "aethos"
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let args = if args.get(1).copied() == Some("aethos") {
        &args[2..]
    } else {
        &args[1..]
    };

    match args {
        ["new", name] => cmd_new(name),
        ["gen", "controller", name] => cmd_gen_controller(name),
        ["gen", "live", name] => cmd_gen_live(name),
        ["gen", "channel", name] => cmd_gen_channel(name),
        ["gen", "migration", name] => cmd_gen_migration(name),
        ["db.migrate"] => cmd_db_migrate("."),
        ["db.rollback"] => cmd_db_rollback("."),
        ["db.reset"] => cmd_db_reset("."),
        ["db.status"] => cmd_db_status("."),
        ["routes"] => cmd_routes("."),
        _ => {
            eprintln!("{}", HELP);
            process::exit(1);
        }
    }
}

const HELP: &str = r#"
cargo aethos — Aethos framework scaffold tool

USAGE:
    cargo aethos new <app_name>              Create a new Aethos application
    cargo aethos gen controller <Name>       Generate a controller
    cargo aethos gen live <Name>             Generate a LiveView
    cargo aethos gen channel <Name>          Generate a Channel
    cargo aethos gen migration <name>        Generate a timestamped migration file

    cargo aethos db.migrate                  Run pending migrations
    cargo aethos db.rollback                 Roll back the last migration
    cargo aethos db.reset                    Drop and recreate the database
    cargo aethos db.status                   Show applied migrations

    cargo aethos routes                      Print all routes defined in src/
"#;

fn cmd_new(name: &str) {
    let dir = Path::new(name);
    if dir.exists() {
        eprintln!("Directory '{name}' already exists.");
        process::exit(1);
    }
    std::fs::create_dir_all(dir.join("src/controllers")).unwrap();

    write_file(
        &dir.join("Cargo.toml"),
        &format!(
            r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"

[dependencies]
aethos = {{ path = "../aethos/crates/aethos" }}
tokio = {{ version = "1", features = ["full"] }}
"#
        ),
    );

    write_file(&dir.join("src/main.rs"), &scaffold_main());
    write_file(&dir.join("src/router.rs"), &scaffold_router());
    write_file(
        &dir.join("src/controllers/page_controller.rs"),
        &scaffold_page_controller(),
    );

    println!("✓ Created new Aethos app: {name}");
    println!("  cd {name} && cargo run");
}

fn cmd_gen_controller(name: &str) {
    let snake = to_snake(name);
    let path = format!("src/controllers/{snake}_controller.rs");
    write_file(Path::new(&path), &scaffold_controller(name));
    println!("✓ Generated controller: {path}");
}

fn cmd_gen_live(name: &str) {
    let snake = to_snake(name);
    std::fs::create_dir_all("src/live").unwrap();
    let path = format!("src/live/{snake}_live.rs");
    write_file(Path::new(&path), &scaffold_live(name));
    println!("✓ Generated LiveView: {path}");
}

fn cmd_gen_channel(name: &str) {
    let snake = to_snake(name);
    std::fs::create_dir_all("src/channels").unwrap();
    let path = format!("src/channels/{snake}_channel.rs");
    write_file(Path::new(&path), &scaffold_channel(name));
    println!("✓ Generated Channel: {path}");
}

/// Scan source files for `router!` macro bodies and extract route definitions.
///
/// Recognises: `get!(`, `post!(`, `put!(`, `patch!(`, `delete!(`,
///             `resources!(`, `live!(`, `websocket!(`.
fn cmd_routes(root: &str) {
    use std::fs;

    #[derive(Debug)]
    struct Route { method: String, path: String, handler: String }

    let mut routes: Vec<Route> = Vec::new();

    // Walk every *.rs file under root/src
    let src = std::path::Path::new(root).join("src");
    let rs_files = find_rs_files(&src);

    let route_re = build_route_regex();

    for file in &rs_files {
        let Ok(content) = fs::read_to_string(file) else { continue };
        for cap in route_re.captures_iter(&content) {
            let method  = cap[1].to_uppercase();
            let path    = cap[2].trim().to_owned();
            let handler = cap.get(3).map_or("", |m| m.as_str()).trim().to_owned();
            let (display_method, display_path, display_handler) = match method.as_str() {
                "RESOURCES" => ("*", path.as_str(), handler.as_str()),
                "LIVE"      => ("WS/GET", path.as_str(), handler.as_str()),
                "WEBSOCKET" => ("WS", path.as_str(), handler.as_str()),
                _           => (method.as_str(), path.as_str(), handler.as_str()),
            };
            routes.push(Route {
                method:  display_method.to_owned(),
                path:    display_path.to_owned(),
                handler: display_handler.to_owned(),
            });
        }
    }

    if routes.is_empty() {
        println!("No routes found. Make sure src/ contains router! macro invocations.");
        return;
    }

    // Pretty-print table
    let col_method  = routes.iter().map(|r| r.method.len()).max().unwrap_or(6).max(6);
    let col_path    = routes.iter().map(|r| r.path.len()).max().unwrap_or(4).max(4);
    let col_handler = routes.iter().map(|r| r.handler.len()).max().unwrap_or(7).max(7);

    let sep = format!(
        "+-{}-+-{}-+-{}-+",
        "-".repeat(col_method),
        "-".repeat(col_path),
        "-".repeat(col_handler),
    );
    println!("{sep}");
    println!(
        "| {:<col_method$} | {:<col_path$} | {:<col_handler$} |",
        "METHOD", "PATH", "HANDLER"
    );
    println!("{sep}");
    for r in &routes {
        println!(
            "| {:<col_method$} | {:<col_path$} | {:<col_handler$} |",
            r.method, r.path, r.handler
        );
    }
    println!("{sep}");
    println!("{} route(s) total.", routes.len());
}

fn find_rs_files(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else { return out };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(find_rs_files(&path));
        } else if path.extension().map_or(false, |e| e == "rs") {
            out.push(path);
        }
    }
    out
}

/// Returns a regex that matches route macro invocations:
/// `get!("/path", Handler, action)` or `live!("/path", Live)`
fn build_route_regex() -> regex::Regex {
    regex::Regex::new(
        r#"(?x)
        (get|post|put|patch|delete|resources|live|websocket)   # method macro name
        \s*!\s*\(                                                # !( opener
        \s*"([^"]+)"                                             # quoted path
        (?:\s*,\s*([^),]+))?                                     # optional handler
        "#
    ).expect("route regex is valid")
}

// ── Templates ─────────────────────────────────────────────────────────────────

fn scaffold_main() -> String {
    r#"mod router;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let router = router::build();
    let endpoint = aethos::Endpoint::new(router);
    let addr = "127.0.0.1:4000".parse()?;
    endpoint.start(addr).await
}
"#
    .into()
}

fn scaffold_router() -> String {
    r#"use aethos::router;
use crate::controllers::page_controller::PageController;

pub fn build() -> axum::Router {
    router! {
        pipeline :browser {
            plug!(aethos::Logger);
            plug!(aethos::SecureHeaders);
        }

        scope "/" {
            pipe_through!(:browser);
            get!("/", PageController, index);
        }
    }
}
"#
    .into()
}

fn scaffold_page_controller() -> String {
    r#"use aethos::Conn;

pub struct PageController;

impl PageController {
    pub async fn index(conn: Conn) -> Conn {
        conn.html("<h1>Welcome to Aethos!</h1>")
    }
}
"#
    .into()
}

fn scaffold_controller(name: &str) -> String {
    format!(
        r#"use aethos::Conn;

pub struct {name}Controller;

impl {name}Controller {{
    pub async fn index(conn: Conn) -> Conn {{
        conn.json(&serde_json::json!({{ "data": [] }}))
    }}

    pub async fn show(conn: Conn) -> Conn {{
        let id = conn.params.get("id").unwrap_or("unknown");
        conn.json(&serde_json::json!({{ "id": id }}))
    }}

    pub async fn create(conn: Conn) -> Conn {{
        conn.json(&serde_json::json!({{ "status": "created" }}))
    }}

    pub async fn update(conn: Conn) -> Conn {{
        conn.json(&serde_json::json!({{ "status": "updated" }}))
    }}

    pub async fn delete(conn: Conn) -> Conn {{
        conn.send_resp(aethos::http::StatusCode::NO_CONTENT, "")
    }}
}}
"#
    )
}

fn scaffold_live(name: &str) -> String {
    format!(
        r#"use aethos::{{LiveView, LiveSocket, Template, Html}};
use async_trait::async_trait;
use std::collections::HashMap;

pub struct {name}Live;

#[async_trait]
impl LiveView for {name}Live {{
    async fn mount(_params: HashMap<String, String>, socket: LiveSocket) -> LiveSocket {{
        socket.assign(Count(0))
    }}

    fn render(socket: &LiveSocket) -> Template {{
        let count = socket.get_assign::<Count>().unwrap_or(Count(0)).0;
        Template::from(Html::new(format!(
            "<div><p>Count: {{}}</p><button phx-click=\"inc\">+</button></div>",
            count
        )))
    }}

    async fn handle_event(event: &str, _payload: serde_json::Value, socket: LiveSocket) -> LiveSocket {{
        match event {{
            "inc" => socket.update(|Count(n): Count| Count(n + 1)),
            _ => socket,
        }}
    }}
}}

#[derive(Clone, Default)]
struct Count(i64);
"#
    )
}

fn scaffold_channel(name: &str) -> String {
    format!(
        r#"use aethos::{{Channel, Socket}};
use aethos_channels::channel::JoinResult;
use async_trait::async_trait;
use serde_json::Value;

pub struct {name}Channel;

#[async_trait]
impl Channel for {name}Channel {{
    async fn join(_topic: &str, _payload: Value, socket: Socket) -> JoinResult {{
        Ok(socket)
    }}

    async fn handle_in(event: &str, payload: Value, socket: Socket) -> Socket {{
        match event {{
            "msg" => {{
                socket.broadcast("msg", &payload);
                socket
            }}
            _ => socket,
        }}
    }}
}}
"#
    )
}

// ── DB / Migration commands ───────────────────────────────────────────────────

fn migration_dir(project_root: &str) -> std::path::PathBuf {
    Path::new(project_root).join("priv").join("repo").join("migrations")
}

fn db_url(project_root: &str) -> String {
    // Read DATABASE_URL from env or fall back to a SQLite dev db
    std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        let db = Path::new(project_root).join("priv").join("repo").join("dev.db");
        format!("sqlite://{}", db.display())
    })
}

fn cmd_gen_migration(name: &str) {
    let ts = {
        // Use a simple numeric timestamp from SystemTime
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        secs
    };
    let filename = format!("{ts}_{name}.sql");
    let dir = migration_dir(".");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(&filename);
    std::fs::write(&path, format!("-- Migration: {name}\n\n")).unwrap();
    println!("Created migration: {}", path.display());
}

fn cmd_db_migrate(project_root: &str) {
    use std::process::Command;
    // We spin up a small async runtime just for migration
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        let url = db_url(project_root);
        let pool = match sqlx::SqlitePool::connect(&url).await {
            Ok(p) => p,
            Err(e) => { eprintln!("db.migrate: cannot connect: {e}"); process::exit(1); }
        };
        let runner = aethos_orm::MigrationRunner::new(migration_dir(project_root));
        if let Err(e) = runner.run(&pool).await {
            eprintln!("db.migrate failed: {e}");
            process::exit(1);
        }
        println!("Migrations complete.");
    });
}

fn cmd_db_rollback(project_root: &str) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        let url = db_url(project_root);
        let pool = match sqlx::SqlitePool::connect(&url).await {
            Ok(p) => p,
            Err(e) => { eprintln!("db.rollback: cannot connect: {e}"); process::exit(1); }
        };
        let runner = aethos_orm::MigrationRunner::new(migration_dir(project_root));
        if let Err(e) = runner.rollback(&pool).await {
            eprintln!("db.rollback failed: {e}");
            process::exit(1);
        }
        println!("Rollback complete.");
    });
}

fn cmd_db_reset(project_root: &str) {
    let url = db_url(project_root);
    // For SQLite, just remove the file and re-migrate
    if url.starts_with("sqlite://") {
        let file = url.trim_start_matches("sqlite://");
        if Path::new(file).exists() {
            std::fs::remove_file(file).unwrap();
            println!("Dropped: {file}");
        }
    } else {
        eprintln!("db.reset only supports SQLite automatically. For Postgres, drop/create the DB manually.");
    }
    cmd_db_migrate(project_root);
}

fn cmd_db_status(project_root: &str) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        let url = db_url(project_root);
        let pool = match sqlx::SqlitePool::connect(&url).await {
            Ok(p) => p,
            Err(e) => { eprintln!("db.status: cannot connect: {e}"); process::exit(1); }
        };
        let runner = aethos_orm::MigrationRunner::new(migration_dir(project_root));
        match runner.status(&pool).await {
            Ok(rows) if rows.is_empty() => println!("No migrations applied."),
            Ok(rows) => {
                println!("{:<50} {}", "Migration", "Applied at");
                println!("{}", "-".repeat(70));
                for (name, ts) in rows { println!("{:<50} {}", name, ts); }
            }
            Err(e) => { eprintln!("db.status failed: {e}"); process::exit(1); }
        }
    });
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn write_file(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, content).unwrap();
}

fn to_snake(name: &str) -> String {
    let mut s = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            s.push('_');
        }
        s.push(c.to_lowercase().next().unwrap());
    }
    s
}
