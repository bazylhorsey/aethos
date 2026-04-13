use std::collections::HashMap;
use std::sync::OnceLock;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use serde_json::Value;

type HmacSha256 = Hmac<Sha256>;

static SESSION_SECRET: OnceLock<String> = OnceLock::new();

/// Configure the session signing secret. Call this before starting the server.
///
/// Defaults to the `AETHOS_SESSION_SECRET` env var, or a dev-only placeholder.
pub fn set_session_secret(key: impl Into<String>) {
    SESSION_SECRET.get_or_init(|| key.into());
}

fn secret() -> &'static str {
    SESSION_SECRET.get_or_init(|| {
        std::env::var("AETHOS_SESSION_SECRET")
            .unwrap_or_else(|_| "aethos-dev-secret-CHANGE-IN-PRODUCTION".to_string())
    })
}

/// Lightweight cookie-based session, analogous to `Plug.Session`.
///
/// Data is JSON-serialized, base64url-encoded, and signed with HMAC-SHA256.
/// The cookie format is: `<base64url(json)>.<base64url(hmac)>`
#[derive(Clone, Default)]
pub struct Session {
    data: HashMap<String, Value>,
    /// Set to `true` whenever data is modified so `into_response` knows to write the cookie.
    pub(crate) dirty: bool,
}

impl Session {
    pub fn new() -> Self {
        Self::default()
    }

    /// Store a JSON-serialisable value in the session.
    pub fn put(&mut self, key: impl Into<String>, value: impl Into<Value>) {
        self.data.insert(key.into(), value.into());
        self.dirty = true;
    }

    /// Retrieve a value from the session.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.data.get(key)
    }

    /// Remove a key from the session.
    pub fn delete(&mut self, key: &str) {
        if self.data.remove(key).is_some() {
            self.dirty = true;
        }
    }

    /// True if the session has been modified and must be written back to the cookie.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    // ── Cookie serialization ─────────────────────────────────────────────────

    /// Encode the session data into a signed cookie value.
    pub fn encode(&self) -> String {
        let json = serde_json::to_string(&self.data).unwrap_or_else(|_| "{}".into());
        let data_b64 = URL_SAFE_NO_PAD.encode(json.as_bytes());
        let sig = sign(&data_b64);
        format!("{data_b64}.{sig}")
    }

    /// Decode and verify a cookie value. Returns `None` if the signature is invalid.
    pub fn decode(cookie_value: &str) -> Option<Self> {
        let (data_b64, sig) = cookie_value.split_once('.')?;
        let expected = sign(data_b64);
        if !constant_time_eq(sig.as_bytes(), expected.as_bytes()) {
            return None;
        }
        let bytes = URL_SAFE_NO_PAD.decode(data_b64).ok()?;
        let data: HashMap<String, Value> = serde_json::from_slice(&bytes).ok()?;
        Some(Self { data, dirty: false })
    }
}

fn sign(data_b64: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(secret().as_bytes())
        .expect("HMAC accepts any key length");
    mac.update(data_b64.as_bytes());
    URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
}

/// Constant-time byte slice comparison to prevent timing attacks.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b.iter()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}
