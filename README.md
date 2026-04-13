# Aethos

A Phoenix-inspired web framework for Rust, built on [Axum](https://github.com/tokio-rs/axum) and Tokio.

Aethos brings Phoenix's best ideas to Rust:

| Phoenix | Aethos |
|---|---|
| `Plug.Conn` | `aethos::Conn` |
| `Plug` behaviour | `aethos::Plug` trait |
| `Phoenix.Router` | `router!{}` macro |
| `~H` HEEx templates | `h!{}` proc macro |
| `Phoenix.PubSub` | `aethos::PubSub` |
| `Phoenix.Channel` | `aethos::Channel` trait |
| `Phoenix.Presence` | `aethos::Presence` |
| `Phoenix.LiveView` | `aethos::LiveView` trait |
| `mix phx.new` | `cargo aethos new` |

---

## Quick Start

```toml
# Cargo.toml
[dependencies]
aethos = { path = "..." }   # crates.io soon
tokio  = { version = "1", features = ["full"] }
```

```rust
use aethos::{router, h, Conn, ConnHtmlExt, Endpoint, Logger, BodyParser};
use std::net::SocketAddr;

pub struct PageController;

impl PageController {
    pub async fn index(conn: Conn) -> Conn {
        conn.render(h! {
            <div>
                <h1>Hello from Aethos!</h1>
            </div>
        })
    }
}

#[tokio::main]
async fn main() {
    let router = router! {
        pipeline :browser {
            plug!(Logger);
            plug!(BodyParser);
        }
        scope "/" {
            pipe_through!(:browser);
            get!("/", PageController, index);
        }
    };
    Endpoint::new(router)
        .start("127.0.0.1:4000".parse().unwrap())
        .await
        .unwrap();
}
```

---

## Features

### `h!` — HEEx-inspired templates

```rust
fn greet_view(name: &str, items: &[&str]) -> Html {
    h! {
        <div class="container">
            <h1>Hello, {name}!</h1>
            <ul>
                <li :for={item in items.iter()}>{item}</li>
            </ul>
            <p :if={items.is_empty()}>No items yet.</p>
        </div>
    }
}
```

- `{expr}` — HTML-escaped by default (XSS-safe)
- `{raw(expr)}` — unescaped trusted HTML
- `{@name}` — shorthand for `assigns.name` inside component functions
- `:if={condition}` — conditional rendering
- `:for={pat in iter}` — list rendering
- `<.component />` — call a local function component
- `<Mod.component />` — call a module-qualified component
- Compile-time tag nesting validation

### `router!` — Pipelines, scopes, resources

```rust
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
        get!("/users/:id", UserController, show);
        resources!("/posts", PostController);  // all 7 REST routes
        live!("/dashboard", DashboardLive);    // LiveView
        websocket!("/chat", ChatChannel);      // WebSocket Channel
    }

    scope "/api/v1" {
        pipe_through!(:api);
        resources!("/users", Api.UserController);
    }
}
```

### Controllers

```rust
pub struct UserController;

impl UserController {
    pub async fn show(conn: Conn) -> Conn {
        let id = conn.params.get("id").unwrap_or("?");
        conn.render(h! { <h1>User {id}</h1> })
    }

    pub async fn create(conn: Conn) -> Conn {
        let name = conn.params.get("name").unwrap_or("unknown");
        conn.json(&serde_json::json!({ "created": true, "name": name }))
    }
}
```

### LiveView

Server-stateful UI over WebSocket. The browser gets the initial HTML via HTTP, then Aethos upgrades to WebSocket and sends only diffs when state changes.

```rust
use aethos::{LiveView, LiveSocket, h, Html, async_trait};
use serde_json::Value;

pub struct CounterLive;

impl Default for CounterLive { fn default() -> Self { Self } }

#[derive(Clone, Default)]
struct Count(i32);

#[async_trait]
impl LiveView for CounterLive {
    async fn mount(_params: std::collections::HashMap<String, String>, socket: LiveSocket) -> LiveSocket {
        socket.assign(Count(0))
    }

    fn render(socket: &LiveSocket) -> Html {
        let count = socket.get_assign::<Count>().map(|c| c.0).unwrap_or(0);
        h! {
            <div>
                <p>Count: {count.to_string()}</p>
                <button phx-click="inc">+</button>
                <button phx-click="dec">-</button>
            </div>
        }
    }

    async fn handle_event(event: &str, _payload: Value, socket: LiveSocket) -> LiveSocket {
        match event {
            "inc" => socket.update::<Count, _>(|c| Count(c.0 + 1)),
            "dec" => socket.update::<Count, _>(|c| Count((c.0 - 1).max(0))),
            _ => socket,
        }
    }
}
```

Register in the router: `live!("/counter", CounterLive)`

The `aethos.js` client (served automatically at `/_aethos/aethos.js`) handles WebSocket connection, heartbeat, DOM patching, and event binding.

### Channels

```rust
use aethos::{Channel, Socket, async_trait};
use aethos::channel::JoinResult;
use serde_json::Value;

pub struct RoomChannel;
impl Default for RoomChannel { fn default() -> Self { Self } }

#[async_trait]
impl Channel for RoomChannel {
    async fn join(_topic: &str, _payload: Value, socket: Socket) -> JoinResult {
        Ok(socket)
    }

    async fn handle_in(event: &str, payload: Value, socket: Socket) -> Socket {
        if event == "new_msg" {
            socket.broadcast("new_msg", &payload);
        }
        socket
    }
}
```

Register: `websocket!("/room/socket", RoomChannel)`

### PubSub

```rust
let pubsub = PubSub::new();
pubsub.broadcast("room:lobby", Message::new("new_msg", &payload));
// in a subscriber:
let mut rx = pubsub.subscribe("room:lobby");
while let Ok(msg) = rx.recv().await { /* ... */ }
```

### Presence

```rust
Presence::track(&socket, "user_1", serde_json::json!({ "name": "Alice" })).await;
let list = Presence::list("room:lobby").await;
// broadcasts presence_state + presence_diff via PubSub
```

### Built-in Plugs

| Plug | Description |
|---|---|
| `Logger` | Logs method, path, status, elapsed time |
| `RequestId` | Adds `X-Request-Id` header |
| `SecureHeaders` | X-Frame-Options, CSP, XSS-Protection |
| `BodyParser` | Parses JSON + form-urlencoded into `conn.params` |

### Scaffold CLI

```bash
cargo install aethos_cli
cargo aethos new my_app
cargo aethos gen controller Users
cargo aethos gen live Dashboard
cargo aethos gen channel Room
```

---

## Workspace Structure

```
crates/
├── aethos/           # public facade (users import this)
├── aethos_core/      # Conn, Plug, params, flash, type_map
├── aethos_router/    # Pipeline, Endpoint
├── aethos_html/      # Html, Assigns, layout, h! re-export
├── aethos_macros/    # router!, h!, #[controller] proc macros
├── aethos_pubsub/    # PubSub (tokio broadcast)
├── aethos_channels/  # Channel trait, Socket, WS transport
├── aethos_presence/  # Presence tracking
├── aethos_live/      # LiveView trait, LiveSocket, WS transport
├── aethos_cli/       # cargo-aethos binary
└── aethos_example/   # full working example app
```

---

## License

MIT
