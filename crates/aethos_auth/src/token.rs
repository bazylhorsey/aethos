use rand::RngCore;
use sha2::{Sha256, Digest};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};

/// Generate a cryptographically random token.
///
/// Returns `(raw_token, hashed_token)`:
/// - Store `hashed_token` in the database.
/// - Return `raw_token` to the client (cookie / API response).
///
/// This mirrors Phoenix's `UserToken.build_session_token/1` — the database
/// never holds the raw value, so a DB breach cannot replay sessions.
pub fn generate_token() -> (String, String) {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    let raw    = URL_SAFE_NO_PAD.encode(bytes);
    let hashed = hash_token(&raw);
    (raw, hashed)
}

/// SHA-256 hash a token for DB lookup.
pub fn hash_token(token: &str) -> String {
    let digest = Sha256::digest(token.as_bytes());
    format!("{:x}", digest)
}
