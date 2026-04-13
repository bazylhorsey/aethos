use futures::{SinkExt, StreamExt};
use axum::extract::ws::{Message, WebSocket};
use serde_json::{json, Value};
use tokio::sync::mpsc;

use crate::{LiveView, LiveSocket, socket::{NavigationAction, StreamOp}};
use aethos_core::telemetry::{Telemetry, elapsed_ms};
use aethos_html::Template;

/// Per-connection LiveView WebSocket task.
///
/// Handles the Phoenix wire protocol over an Axum WebSocket:
/// - `phx_join`   → mount the LiveView and send initial rendered template
/// - `event`      → dispatch to `handle_event`, send binary diff (only changed slots)
/// - `heartbeat`  → respond with pong
/// - server-initiated updates via `handle_info` (through an internal channel)
///
/// The inner loop runs in a spawned task so that panics in user LiveView code
/// (mount, render, handle_event) are caught, logged, and result in a clean
/// WebSocket close rather than silently killing an Axum worker thread.
pub async fn handle_live_socket<LV>(socket: WebSocket, params: std::collections::HashMap<String, String>)
where
    LV: LiveView + Default,
{
    let handle = tokio::task::spawn(live_socket_loop::<LV>(socket, params));
    match handle.await {
        Ok(()) => {}
        Err(e) if e.is_panic() => {
            tracing::error!(
                view = std::any::type_name::<LV>(),
                "LiveView connection panicked — client will reconnect: {e:?}"
            );
        }
        Err(e) => {
            tracing::warn!(view = std::any::type_name::<LV>(), "LiveView task cancelled: {e:?}");
        }
    }
}

async fn live_socket_loop<LV>(socket: WebSocket, params: std::collections::HashMap<String, String>)
where
    LV: LiveView + Default,
{
    let (mut ws_tx, mut ws_rx) = socket.split();
    let (info_tx, mut info_rx) = mpsc::unbounded_channel::<Value>();

    let mut live_socket: Option<LiveSocket> = None;
    let mut prev_template: Option<Template> = None;
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
                            &mut prev_template,
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
                    let (new_socket, push) = build_push::<LV>(
                        new_socket, &mut prev_template, &joined_topic, None, None,
                    );
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

/// Build the JSON diff payload comparing `next` against `prev`.
///
/// - First render (prev = None): returns `{"s": [...statics...], "0": val, "1": val, …}`
/// - Same structure: returns only changed slots `{"1": new_val}` (may be `{}` if nothing changed)
/// - Structure changed (different number of dynamic slots): returns full initial render
fn template_diff_json(next: &Template, prev: Option<&Template>) -> Value {
    match prev {
        None => template_initial_json(next),
        Some(prev) => match next.diff_pairs(prev) {
            None => {
                // Structural change — full re-render with statics
                template_initial_json(next)
            }
            Some(pairs) => {
                let mut obj = serde_json::Map::new();
                for (i, val) in pairs {
                    obj.insert(i.to_string(), Value::String(val.to_owned()));
                }
                Value::Object(obj)
            }
        },
    }
}

/// Full initial render JSON: `{"s": [...statics...], "0": val, "1": val, …}`.
fn template_initial_json(t: &Template) -> Value {
    let mut obj = serde_json::Map::new();
    let statics: Vec<Value> = t.statics.iter()
        .map(|s| Value::String(s.to_string()))
        .collect();
    obj.insert("s".into(), Value::Array(statics));
    for (i, d) in t.dynamics.iter().enumerate() {
        obj.insert(i.to_string(), Value::String(d.clone()));
    }
    Value::Object(obj)
}

/// Build a push message from the socket state after an update.
/// Returns the (possibly updated) socket and an optional message to send.
fn build_push<LV: LiveView>(
    mut socket: LiveSocket,
    prev_template: &mut Option<Template>,
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

    // Compute template diff
    let new_template = LV::render(&socket);
    let diff = template_diff_json(&new_template, prev_template.as_ref());

    // Only send if something changed (or no prev template yet)
    let has_changes = prev_template.as_ref()
        .map(|p| !new_template.is_same_as(p))
        .unwrap_or(true);

    if has_changes {
        let push = json!([join_ref, msg_ref, topic, "diff", diff]);
        *prev_template = Some(new_template);
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
    prev_template: &mut Option<Template>,
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
            let mut sock = LV::handle_params(params.clone(), &url, sock).await;

            let template = LV::render(&sock);
            let flash_msgs = sock.take_flash_msgs();

            let mut rendered = template_initial_json(&template);
            if !flash_msgs.is_empty() {
                let events: Vec<Value> = flash_msgs.iter().map(|f| {
                    json!({"event": "put-flash", "payload": {"key": f.key, "msg": f.msg}})
                }).collect();
                rendered["e"] = Value::Array(events);
            }

            let reply = json!([
                join_ref, msg_ref, topic, "phx_reply",
                {"status": "ok", "response": {"rendered": rendered}}
            ]);
            ws_tx.send(Message::Text(reply.to_string().into())).await?;
            *prev_template = Some(template);
            *live_socket = Some(sock);

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
                let mut new_sock = LV::handle_event(ev_name, ev_value, sock).await;

                let mut meta = std::collections::HashMap::new();
                meta.insert("view".into(), std::any::type_name::<LV>().into());
                meta.insert("event".into(), ev_name.to_owned());
                Telemetry::duration("aethos.live_view.handle_event", elapsed_ms(start), meta);

                // Navigation short-circuit
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

                let flash_msgs = new_sock.take_flash_msgs();
                let stream_ops = new_sock.take_stream_ops();

                let mut diff = if !stream_ops.is_empty() {
                    json!({"streams": build_streams_payload(&stream_ops)})
                } else {
                    let new_template = LV::render(&new_sock);
                    let d = template_diff_json(&new_template, prev_template.as_ref());
                    *prev_template = Some(new_template);
                    d
                };

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

