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
        _ => {
            eprintln!("{}", HELP);
            process::exit(1);
        }
    }
}

const HELP: &str = r#"
cargo aethos — Aethos framework scaffold tool

USAGE:
    cargo aethos new <app_name>           Create a new Aethos application
    cargo aethos gen controller <Name>    Generate a controller
    cargo aethos gen live <Name>          Generate a LiveView
    cargo aethos gen channel <Name>       Generate a Channel
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
        r#"use aethos::{{LiveView, LiveSocket, Html}};
use async_trait::async_trait;
use std::collections::HashMap;

pub struct {name}Live;

#[async_trait]
impl LiveView for {name}Live {{
    async fn mount(_params: HashMap<String, String>, socket: LiveSocket) -> LiveSocket {{
        socket.assign(Count(0))
    }}

    fn render(socket: &LiveSocket) -> Html {{
        let count = socket.get_assign::<Count>().unwrap_or(Count(0)).0;
        Html::new(format!(
            "<div><p>Count: {{}}</p><button phx-click=\"inc\">+</button></div>",
            count
        ))
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
