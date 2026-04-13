/// Aethos example application.
///
/// Demonstrates:
/// - router! with pipelines, scopes, and resources
/// - Two controllers (PageController, GreetController)
/// - CounterLive: a LiveView with phx-click events
/// - RoomChannel: a WebSocket channel with broadcast
/// - Presence tracking
use aethos::{
    router,
    h,
    Conn,
    ConnHtmlExt,
    Html,
    Endpoint,
    Logger,
    BodyParser,
    SecureHeaders,
    LiveView,
    LiveSocket,
    Channel,
    Socket,
};
use aethos::channel::JoinResult;
use aethos::async_trait;
use aethos::serde_json::{self, Value};
use std::net::SocketAddr;

// ── Controllers ───────────────────────────────────────────────────────────────

pub struct PageController;

impl PageController {
    pub async fn index(conn: Conn) -> Conn {
        let html = h! {
            <div class="container">
                <h1>Welcome to Aethos</h1>
                <p>A Phoenix-inspired web framework for Rust.</p>
                <nav>
                    <a href="/greet/world">Say hello</a>
                    <a href="/counter">Live Counter</a>
                </nav>
            </div>
        };
        conn.render(html)
    }
}

pub struct GreetController;

impl GreetController {
    pub async fn show(conn: Conn) -> Conn {
        let name = conn.params.get("name").unwrap_or("stranger").to_owned();
        let html = h! {
            <div>
                <h1>Hello, {name}!</h1>
                <a href="/">Back home</a>
            </div>
        };
        conn.render(html)
    }
}

pub struct UserController;

impl UserController {
    pub async fn index(conn: Conn) -> Conn {
        conn.json(&serde_json::json!([
            {"id": 1, "name": "Alice"},
            {"id": 2, "name": "Bob"},
        ]))
    }

    pub async fn show(conn: Conn) -> Conn {
        let id = conn.params.get("id").unwrap_or("0").to_owned();
        conn.json(&serde_json::json!({"id": id, "name": "Alice"}))
    }

    pub async fn create(conn: Conn) -> Conn {
        let name = conn.params.get("name").unwrap_or("unknown").to_owned();
        conn.json(&serde_json::json!({"created": true, "name": name}))
    }

    pub async fn new(conn: Conn)    -> Conn { conn.json(&serde_json::json!({"form": "create user"})) }
    pub async fn edit(conn: Conn)   -> Conn { conn.json(&serde_json::json!({"form": "edit user"})) }
    pub async fn update(conn: Conn) -> Conn { conn.json(&serde_json::json!({"updated": true})) }
    pub async fn delete(conn: Conn) -> Conn { conn.json(&serde_json::json!({"deleted": true})) }
}

// ── LiveView ──────────────────────────────────────────────────────────────────

pub struct CounterLive;

impl Default for CounterLive {
    fn default() -> Self { Self }
}

#[async_trait]
impl LiveView for CounterLive {
    async fn mount(_params: std::collections::HashMap<String, String>, socket: LiveSocket) -> LiveSocket {
        socket.assign(Count(0))
    }

    fn render(socket: &LiveSocket) -> Html {
        let count = socket.get_assign::<Count>().map(|c| c.0).unwrap_or(0);
        h! {
            <div class="counter">
                <h2>Counter: {count.to_string()}</h2>
                <button phx-click="inc">+ Increment</button>
                <button phx-click="dec">- Decrement</button>
                <button phx-click="reset">Reset</button>
            </div>
        }
    }

    async fn handle_event(event: &str, _payload: Value, socket: LiveSocket) -> LiveSocket {
        match event {
            "inc"   => socket.update::<Count, _>(|c| Count(c.0 + 1)),
            "dec"   => socket.update::<Count, _>(|c| Count((c.0 - 1).max(0))),
            "reset" => socket.assign(Count(0)),
            _       => socket,
        }
    }
}

#[derive(Clone, Default)]
struct Count(i32);

// ── Channel ───────────────────────────────────────────────────────────────────

pub struct RoomChannel;

impl Default for RoomChannel {
    fn default() -> Self { Self }
}

#[async_trait]
impl Channel for RoomChannel {
    async fn join(_topic: &str, _payload: Value, socket: Socket) -> JoinResult {
        Ok(socket)
    }

    async fn handle_in(event: &str, payload: Value, socket: Socket) -> Socket {
        match event {
            "new_msg" => {
                socket.broadcast("new_msg", &payload);
                socket
            }
            _ => socket,
        }
    }
}

// ── Router ────────────────────────────────────────────────────────────────────

fn build_router() -> aethos::axum::Router {
    router! {
        pipeline :browser {
            plug!(Logger);
            plug!(BodyParser);
            plug!(SecureHeaders);
        }
        pipeline :api {
            plug!(Logger);
            plug!(BodyParser);
        }

        scope "/" {
            pipe_through!(:browser);
            get!("/", PageController, index);
            get!("/greet/:name", GreetController, show);
            live!("/counter", CounterLive);
            websocket!("/room/socket", RoomChannel);
        }

        scope "/api" {
            pipe_through!(:api);
            resources!("/users", UserController);
        }
    }
}

// ── Entry point ───────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    let router = build_router();
    let addr   = SocketAddr::from(([127, 0, 0, 1], 4000));
    Endpoint::new(router).start(addr).await.expect("server failed");
}
