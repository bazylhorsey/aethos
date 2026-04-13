use async_trait::async_trait;
use serde_json::Value;
use thiserror::Error;

use crate::Socket;

/// Result of a channel join attempt.
pub type JoinResult = Result<Socket, JoinError>;

#[derive(Debug, Error)]
pub enum JoinError {
    #[error("unauthorized")]
    Unauthorized,
    #[error("join error: {0}")]
    Custom(String),
}

/// A Phoenix-style channel handler.
///
/// Implement this trait and annotate with `#[channel("topic:pattern")]`.
///
/// ```rust
/// pub struct RoomChannel;
///
/// #[async_trait]
/// impl Channel for RoomChannel {
///     async fn join(topic: &str, payload: Value, socket: Socket) -> JoinResult {
///         Ok(socket.assign(UserId(42)))
///     }
///     async fn handle_in(event: &str, payload: Value, socket: Socket) -> Socket {
///         match event {
///             "new_msg" => { socket.broadcast("new_msg", &payload); socket }
///             _ => socket,
///         }
///     }
/// }
/// ```
#[async_trait]
pub trait Channel: Send + Sync + 'static {
    /// Called when a client joins this topic. Return `Ok(socket)` to allow,
    /// `Err(JoinError)` to reject.
    async fn join(topic: &str, payload: Value, socket: Socket) -> JoinResult
    where
        Self: Sized;

    /// Handle an inbound event from the client.
    async fn handle_in(event: &str, payload: Value, socket: Socket) -> Socket
    where
        Self: Sized;

    /// Called when the socket terminates (client disconnected).
    async fn terminate(_reason: &str, _socket: Socket)
    where
        Self: Sized,
    {
    }
}
