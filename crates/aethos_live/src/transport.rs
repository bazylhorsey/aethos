use futures::{SinkExt, StreamExt};
use axum::extract::ws::{Message, WebSocket};
use serde_json::{json, Value};
use tokio::sync::mpsc;

use crate::{LiveView, LiveSocket};

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
                    let new_html = LV::render(&new_socket).0;

                    if new_html != last_html {
                        let push = json!([null, null, joined_topic, "diff", {"0": new_html}]);
                        if ws_tx.send(Message::Text(push.to_string().into())).await.is_err() {
                            break;
                        }
                        last_html = new_html;
                    }
                    live_socket = Some(new_socket);
                }
            }
        }
    }
}

type WsTx = futures::stream::SplitSink<WebSocket, Message>;

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
            let sock = LiveSocket::new(true);
            let sock = LV::mount(params.clone(), sock).await;
            let html = LV::render(&sock).0;

            let reply = json!([
                join_ref, msg_ref, topic, "phx_reply",
                {
                    "status": "ok",
                    "response": {
                        "rendered": {
                            "0": html,
                            "s": ["", ""]
                        }
                    }
                }
            ]);
            ws_tx.send(Message::Text(reply.to_string().into())).await?;
            *last_html = html;
            *live_socket = Some(sock);
        }

        // ── Browser events ────────────────────────────────────────────────────
        "event" => {
            if let Some(sock) = live_socket.take() {
                let ev_name  = payload["event"].as_str().unwrap_or("");
                let ev_value = payload["value"].clone();

                let new_sock = LV::handle_event(ev_name, ev_value, sock).await;
                let new_html = LV::render(&new_sock).0;

                let diff = if new_html != *last_html {
                    json!({"0": new_html})
                } else {
                    json!({})
                };

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
