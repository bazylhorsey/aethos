use axum::extract::ws::{Message as WsMessage, WebSocket};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::mpsc;
use tracing::{debug, warn};

use aethos_pubsub::PubSub;

use crate::socket::{Socket, SocketMessage};

/// Wire-format envelope for messages between client and server.
#[derive(Debug, Serialize, Deserialize)]
struct Envelope {
    /// join_ref for tracking channel joins (nullable)
    join_ref: Option<String>,
    /// message ref
    msg_ref: Option<String>,
    /// channel topic
    topic: String,
    /// event name
    event: String,
    /// payload
    payload: Value,
}

/// Handles a raw WebSocket connection for a `UserSocket`-style multiplexed socket.
/// This is a simplified single-channel transport — full multiplexing will be
/// expanded in a later phase.
pub async fn handle_socket<S: crate::Channel>(_channel: S, mut ws: WebSocket) {
    let pubsub = PubSub::new();
    let (reply_tx, mut reply_rx) = mpsc::unbounded_channel::<SocketMessage>();

    let mut socket_opt: Option<Socket> = None;

    loop {
        tokio::select! {
            // Inbound from browser
            msg = ws.next() => {
                let msg = match msg {
                    Some(Ok(m)) => m,
                    _ => break,
                };
                let text = match msg {
                    WsMessage::Text(t) => t,
                    WsMessage::Close(_) => break,
                    _ => continue,
                };

                let env: Envelope = match serde_json::from_str(&text) {
                    Ok(e) => e,
                    Err(e) => { warn!("invalid envelope: {e}"); continue; }
                };

                debug!(event = %env.event, topic = %env.topic, "channel inbound");

                match env.event.as_str() {
                    "phx_join" => {
                        let sock = Socket::new_with_channel(
                            env.topic.clone(),
                            reply_tx.clone(),
                            pubsub.clone(),
                        );
                        match S::join(&env.topic, env.payload, sock).await {
                            Ok(s) => {
                                // send phx_reply ok
                                let reply = Envelope {
                                    join_ref: env.join_ref.clone(),
                                    msg_ref: env.msg_ref,
                                    topic: env.topic,
                                    event: "phx_reply".into(),
                                    payload: serde_json::json!({ "status": "ok", "response": {} }),
                                };
                                let _ = ws.send(WsMessage::Text(
                                    serde_json::to_string(&reply).unwrap().into(),
                                )).await;
                                socket_opt = Some(s);
                            }
                            Err(e) => {
                                let reply = Envelope {
                                    join_ref: env.join_ref,
                                    msg_ref: env.msg_ref,
                                    topic: env.topic,
                                    event: "phx_reply".into(),
                                    payload: serde_json::json!({ "status": "error", "response": { "reason": e.to_string() } }),
                                };
                                let _ = ws.send(WsMessage::Text(
                                    serde_json::to_string(&reply).unwrap().into(),
                                )).await;
                            }
                        }
                    }
                    "phx_leave" => break,
                    event => {
                        if let Some(sock) = socket_opt.take() {
                            let sock = S::handle_in(event, env.payload, sock).await;
                            socket_opt = Some(sock);
                        }
                    }
                }
            }

            // Outbound replies from channel
            reply = reply_rx.recv() => {
                if let Some(r) = reply {
                    let env = Envelope {
                        join_ref: None,
                        msg_ref: None,
                        topic: r.topic,
                        event: r.event,
                        payload: r.payload,
                    };
                    let _ = ws.send(WsMessage::Text(
                        serde_json::to_string(&env).unwrap().into(),
                    )).await;
                }
            }
        }
    }

    if let Some(sock) = socket_opt {
        S::terminate("left", sock).await;
    }
}
