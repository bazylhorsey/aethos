use futures::{SinkExt, StreamExt};
use axum::extract::ws::{Message, WebSocket};
use serde_json::{json, Value};
use tokio::sync::mpsc;

use crate::{LiveView, LiveSocket, socket::{NavigationAction, StreamOp}};
use aethos_core::telemetry::{Telemetry, elapsed_ms};

/// Per-connection LiveView WebSocket task.
///
/// Handles the Phoenix wire protocol over an Axum WebSocket:
/// - `phx_join`   → mount the LiveView and send initial rendered HTML
/// - `event`      → dispatch to `handle_event`, send back diff
/// - `heartbeat`  → respond with pong
/// - server-initiated updates via `handle_info` (through an internal channel)
pub async fn handle_live_socket<LV>(socket: WebSocket, params: std::collections::HashMap<String, String>)
where
    LV: LiveView + Default,
{
    let (mut ws_tx, mut ws_rx) = socket.split();
    let (info_tx, mut info_rx) = mpsc::unbounded_channel::<Value>();

    let mut live_socket: Option<LiveSocket> = None;
    let mut last_html: String = String::new();
    let mut joined_topic: String = String::new();

    loop {
        tokio::select! {
            // ── Inbound browser messages ──────────────────────────────────────
            msg = ws_rx.next() => {
                match msg {
                    None | Some(Err(_)) => break,
                    Some(Ok(Message::Close(_))) => break,
                    Some(Ok(Message::Text(text))) => {
                        if let Err(e) = handle_message::<LV>(
                            &text,
                            &mut ws_tx,
                            &mut live_socket,
                            &mut last_html,
                            &mut joined_topic,
                            &params,
                            info_tx.clone(),
                        ).await {
                            tracing::warn!("LiveView message error: {e}");
                        }
                    }
                    Some(Ok(Message::Ping(d))) => {
                        let _ = ws_tx.send(Message::Pong(d)).await;
                    }
                    _ => {}
                }
            }

            // ── Internal messages (PubSub / timers) ──────────────────────────
            Some(msg) = info_rx.recv() => {
                if let Some(socket) = live_socket.take() {
                    let new_socket = LV::handle_info(msg, socket).await;
                    let (new_socket, push) = build_push::<LV>(new_socket, &mut last_html, &joined_topic, None, None);
                    if let Some(push) = push {
                        if ws_tx.send(Message::Text(push.to_string().into())).await.is_err() {
                            break;
                        }
                    }
                    live_socket = Some(new_socket);
                }
            }
        }
    }
}

type WsTx = futures::stream::SplitSink<WebSocket, Message>;

/// Build a push message from the socket state after an update.
/// Returns the (possibly updated) socket and an optional message to send.
fn build_push<LV: LiveView>(
    mut socket: LiveSocket,
    last_html: &mut String,
    topic: &str,
    join_ref: Option<&Value>,
    msg_ref: Option<&Value>,
) -> (LiveSocket, Option<Value>) {
    // Check for navigation first
    if let Some(nav) = socket.take_navigation() {
        let (event, url) = match nav {
            NavigationAction::Navigate(url) => ("phx_navigate", url),
            NavigationAction::Patch(url)    => ("phx_patch", url),
        };
        let push = json!([join_ref, msg_ref, topic, event, {"to": url}]);
        return (socket, Some(push));
    }

    // Check for stream ops
    let stream_ops = socket.take_stream_ops();
    if !stream_ops.is_empty() {
        let streams = build_streams_payload(&stream_ops);
        let push = json!([join_ref, msg_ref, topic, "diff", {"streams": streams}]);
        return (socket, Some(push));
    }

    // Normal diff
    let new_html = LV::render(&socket).0;
    if new_html != *last_html {
        let push = json!([join_ref, msg_ref, topic, "diff", {"0": new_html}]);
        *last_html = new_html;
        (socket, Some(push))
    } else {
        (socket, None)
    }
}

fn build_streams_payload(ops: &[StreamOp]) -> Value {
    let entries: Vec<Value> = ops.iter().map(|op| match op {
        StreamOp::Insert { name, id, item } => json!({
            "op":   "insert",
            "name": name,
            "id":   id,
            "item": item,
        }),
        StreamOp::Delete { name, id } => json!({
            "op":   "delete",
            "name": name,
            "id":   id,
        }),
        StreamOp::Reset { name } => json!({
            "op":   "reset",
            "name": name,
        }),
    }).collect();
    Value::Array(entries)
}

async fn handle_message<LV>(
    text: &str,
    ws_tx: &mut WsTx,
    live_socket: &mut Option<LiveSocket>,
    last_html: &mut String,
    joined_topic: &mut String,
    params: &std::collections::HashMap<String, String>,
    _info_tx: mpsc::UnboundedSender<Value>,
) -> anyhow::Result<()>
where
    LV: LiveView + Default,
{
    // Phoenix envelope: [join_ref, msg_ref, topic, event, payload]
    let envelope: Value = serde_json::from_str(text)?;
    let arr = envelope.as_array().ok_or_else(|| anyhow::anyhow!("not an array"))?;
    if arr.len() != 5 {
        return Ok(());
    }

    let join_ref = &arr[0];
    let msg_ref  = &arr[1];
    let topic    = arr[2].as_str().unwrap_or("").to_owned();
    let event    = arr[3].as_str().unwrap_or("");
    let payload  = &arr[4];

    match event {
        // ── Mount ─────────────────────────────────────────────────────────────
        "phx_join" => {
            *joined_topic = topic.clone();
            let start = std::time::Instant::now();
            let url = payload["url"].as_str().unwrap_or("/").to_owned();

            let sock = LiveSocket::new(true);
            let sock = LV::mount(params.clone(), sock).await;
            // Call handle_params after mount
            let mut sock = LV::handle_params(params.clone(), &url, sock).await;

            let html = LV::render(&sock).0;
            let flash_msgs = sock.take_flash_msgs();

            // Build events array for flash messages
            let events: Vec<Value> = flash_msgs.iter().map(|f| {
                json!({"event": "put-flash", "payload": {"key": f.key, "msg": f.msg}})
            }).collect();

            let mut rendered = json!({"0": html, "s": ["", ""]});
            if !events.is_empty() {
                rendered["e"] = Value::Array(events);
            }

            let reply = json!([
                join_ref, msg_ref, topic, "phx_reply",
                {"status": "ok", "response": {"rendered": rendered}}
            ]);
            ws_tx.send(Message::Text(reply.to_string().into())).await?;
            *last_html = html;
            *live_socket = Some(sock);

            // Emit telemetry
            let mut meta = std::collections::HashMap::new();
            meta.insert("view".into(), std::any::type_name::<LV>().into());
            Telemetry::duration("aethos.live_view.mount", elapsed_ms(start), meta);
        }

        // ── Browser events ────────────────────────────────────────────────────
        "event" => {
            if let Some(sock) = live_socket.take() {
                let ev_name  = payload["event"].as_str().unwrap_or("");
                let ev_value = payload["value"].clone();

                let start = std::time::Instant::now();
                let new_sock = LV::handle_event(ev_name, ev_value, sock).await;
                let new_html = LV::render(&new_sock).0;

                // Emit telemetry
                let mut meta = std::collections::HashMap::new();
                meta.insert("view".into(), std::any::type_name::<LV>().into());
                meta.insert("event".into(), ev_name.to_owned());
                Telemetry::duration("aethos.live_view.handle_event", elapsed_ms(start), meta);

                // Check for navigation
                let mut new_sock = new_sock;
                if let Some(nav) = new_sock.take_navigation() {
                    let (nav_event, url) = match nav {
                        NavigationAction::Navigate(url) => ("phx_navigate", url),
                        NavigationAction::Patch(url)    => ("phx_patch", url),
                    };
                    let reply = json!([join_ref, msg_ref, topic, "phx_reply",
                        {"status": "ok", "response": {"diff": {}}}
                    ]);
                    ws_tx.send(Message::Text(reply.to_string().into())).await?;
                    let nav_push = json!([null, null, topic, nav_event, {"to": url}]);
                    ws_tx.send(Message::Text(nav_push.to_string().into())).await?;
                    *live_socket = Some(new_sock);
                    return Ok(());
                }

                // Collect flash messages
                let flash_msgs = new_sock.take_flash_msgs();
                let stream_ops = new_sock.take_stream_ops();

                let mut diff = if !stream_ops.is_empty() {
                    json!({"streams": build_streams_payload(&stream_ops)})
                } else if new_html != *last_html {
                    json!({"0": new_html})
                } else {
                    json!({})
                };

                // Embed flash events in diff
                if !flash_msgs.is_empty() {
                    let events: Vec<Value> = flash_msgs.iter().map(|f| {
                        json!({"event": "put-flash", "payload": {"key": f.key, "msg": f.msg}})
                    }).collect();
                    diff["e"] = Value::Array(events);
                }

                let reply = json!([
                    join_ref, msg_ref, topic, "phx_reply",
                    {"status": "ok", "response": {"diff": diff}}
                ]);
                ws_tx.send(Message::Text(reply.to_string().into())).await?;
                *last_html = new_html;
                *live_socket = Some(new_sock);
            }
        }

        // ── Heartbeat ─────────────────────────────────────────────────────────
        "heartbeat" => {
            let pong = json!([null, msg_ref, "phoenix", "phx_reply",
                {"status": "ok", "response": {}}
            ]);
            ws_tx.send(Message::Text(pong.to_string().into())).await?;
        }

        // ── Leave ─────────────────────────────────────────────────────────────
        "phx_leave" => {
            let reply = json!([join_ref, msg_ref, topic, "phx_reply",
                {"status": "ok", "response": {}}
            ]);
            ws_tx.send(Message::Text(reply.to_string().into())).await?;
        }

        _ => {}
    }

    Ok(())
}

