use aethos::{Session, Csrf, CsrfToken, Plug, Next, Conn};
use axum::body::Body;
use axum::http::{Request, StatusCode};

fn make_conn(method: &str, path: &str, cookie: Option<&str>) -> Conn {
    let mut builder = Request::builder()
        .method(method)
        .uri(path);
    if let Some(c) = cookie {
        builder = builder.header("cookie", c);
    }
    let req = builder.body(Body::empty()).unwrap();
    Conn::new(req)
}

// ── Session tests ─────────────────────────────────────────────────────────────

#[test]
fn session_roundtrip() {
    let mut session = Session::new();
    session.put("user_id", serde_json::json!(42));
    session.put("name", serde_json::json!("Alice"));

    let encoded = session.encode();
    let decoded = Session::decode(&encoded).expect("decode should succeed");

    assert_eq!(decoded.get("user_id"), Some(&serde_json::json!(42)));
    assert_eq!(decoded.get("name"), Some(&serde_json::json!("Alice")));
    assert_eq!(decoded.get("missing"), None);
}

#[test]
fn session_tampered_cookie_rejected() {
    let mut session = Session::new();
    session.put("role", serde_json::json!("admin"));
    let encoded = session.encode();

    // Corrupt the signature
    let tampered = format!("{encoded}X");
    assert!(Session::decode(&tampered).is_none());
}

#[test]
fn session_invalid_format_rejected() {
    assert!(Session::decode("notvalid").is_none());
    assert!(Session::decode("").is_none());
    assert!(Session::decode("a.b.c").is_none());
}

#[test]
fn session_dirty_flag() {
    let mut session = Session::new();
    assert!(!session.is_dirty());
    session.put("key", serde_json::json!("val"));
    assert!(session.is_dirty());
}

#[test]
fn session_delete() {
    let mut session = Session::new();
    session.put("key", serde_json::json!("val"));
    session.delete("key");
    assert_eq!(session.get("key"), None);
}

// ── Csrf plug tests ───────────────────────────────────────────────────────────

#[tokio::test]
async fn csrf_get_generates_token() {
    let conn = make_conn("GET", "/", None);
    // Simulate FetchSession + Csrf plugs running (directly manipulate)
    let csrf = Csrf::default();
    let next = Next::terminal();
    let result = csrf.call(conn, next).await;

    // Token should be in assigns
    assert!(result.assigns.get::<CsrfToken>().is_some());
    // Session should be dirty (token was generated)
    assert!(result.session.is_dirty());
}

#[tokio::test]
async fn csrf_post_valid_token_passes() {
    // Seed a session with a known CSRF token
    let mut session = Session::new();
    session.put("_csrf_token", serde_json::json!("test-token-abc123"));

    let req = Request::builder()
        .method("POST")
        .uri("/submit")
        .body(Body::empty())
        .unwrap();
    let mut conn = Conn::new(req);
    conn.session = session;
    conn.params.insert("_csrf_token".to_string(), "test-token-abc123".to_string());

    let csrf = Csrf::default();
    let next = Next::terminal();
    let result = csrf.call(conn, next).await;

    assert_eq!(result.status, StatusCode::OK);
    assert!(!result.halted);
}

#[tokio::test]
async fn csrf_post_missing_token_rejected() {
    let mut session = Session::new();
    session.put("_csrf_token", serde_json::json!("test-token-abc123"));

    let req = Request::builder()
        .method("POST")
        .uri("/submit")
        .body(Body::empty())
        .unwrap();
    let mut conn = Conn::new(req);
    conn.session = session;
    // No _csrf_token in params

    let csrf = Csrf::default();
    let next = Next::terminal();
    let result = csrf.call(conn, next).await;

    assert_eq!(result.status, StatusCode::FORBIDDEN);
    assert!(result.halted);
}

#[tokio::test]
async fn csrf_post_wrong_token_rejected() {
    let mut session = Session::new();
    session.put("_csrf_token", serde_json::json!("correct-token"));

    let req = Request::builder()
        .method("POST")
        .uri("/submit")
        .body(Body::empty())
        .unwrap();
    let mut conn = Conn::new(req);
    conn.session = session;
    conn.params.insert("_csrf_token".to_string(), "wrong-token".to_string());

    let csrf = Csrf::default();
    let next = Next::terminal();
    let result = csrf.call(conn, next).await;

    assert_eq!(result.status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn csrf_post_header_token_accepted() {
    let mut session = Session::new();
    session.put("_csrf_token", serde_json::json!("header-token"));

    let req = Request::builder()
        .method("DELETE")
        .uri("/resource/1")
        .header("x-csrf-token", "header-token")
        .body(Body::empty())
        .unwrap();
    let mut conn = Conn::new(req);
    conn.session = session;

    let csrf = Csrf::default();
    let next = Next::terminal();
    let result = csrf.call(conn, next).await;

    assert_eq!(result.status, StatusCode::OK);
    assert!(!result.halted);
}
