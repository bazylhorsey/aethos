use aethos::{h, path};
use aethos::Assigns;

#[test]
fn renders_plain_div() {
    let html = h! { <div class="hello">World</div> };
    assert!(html.as_str().contains("<div"), "missing open tag");
    assert!(html.as_str().contains("World"), "missing text");
    assert!(html.as_str().contains("</div>"), "missing close tag");
}

#[test]
fn escapes_dynamic_expr() {
    let evil = "<script>alert(1)</script>";
    let html = h! { <p>{evil}</p> };
    assert!(!html.as_str().contains("<script>"), "XSS leak");
    assert!(html.as_str().contains("&lt;script&gt;"), "not escaped");
}

#[test]
fn if_false_renders_nothing() {
    let show = false;
    let html = h! { <span :if={show}>Secret</span> };
    assert!(!html.as_str().contains("Secret"));
}

#[test]
fn for_renders_multiple() {
    let items = vec!["a", "b", "c"];
    let html = h! { <ul><li :for={item in items.iter()}>{item}</li></ul> };
    let s = html.as_str();
    assert!(s.contains("a") && s.contains("b") && s.contains("c"));
}

#[test]
fn static_attrs_emit_correctly() {
    let html = h! { <a href="/home">Home</a> };
    let s = html.as_str();
    assert!(s.contains(r#"href="/home""#));
    assert!(s.contains("Home"));
}

#[test]
fn self_closing_tags() {
    let html = h! { <input type="text" /> };
    assert!(html.as_str().contains("<input"));
    assert!(html.as_str().contains("/>"));
}

// ── Named slots ────────────────────────────────────────────────────────────────

fn card(assigns: &Assigns) -> aethos::Html {
    h! {
        <div class="card">
            <div class="card-header">{raw(assigns.slot("header"))}</div>
            <div class="card-body">{raw(assigns.slot("inner_block"))}</div>
        </div>
    }
}

#[test]
fn named_slot_rendered_in_component() {
    let html = h! {
        <.card>
            <:header><strong>My Title</strong></:header>
            <p>Body content</p>
        </.card>
    };
    let s = html.as_str();
    assert!(s.contains("card-header"), "missing header div");
    assert!(s.contains("<strong>") && s.contains("My") && s.contains("Title"), "slot content missing");
    assert!(s.contains("card-body"), "missing body div");
    assert!(s.contains("Body") && s.contains("content"), "inner_block missing");
}

#[test]
fn self_closing_component_no_slots() {
    fn banner(assigns: &Assigns) -> aethos::Html {
        h! { <div class="banner">{raw(assigns.slot("inner_block"))}</div> }
    }
    let html = h! { <.banner /> };
    assert!(html.as_str().contains("banner"));
}

#[test]
fn slot_absent_returns_empty() {
    let a = Assigns::new();
    let slot = a.slot("missing");
    assert_eq!(slot.as_str(), "");
}

// ── path! macro ───────────────────────────────────────────────────────────────

#[test]
fn path_no_params() {
    let p = path!("/users");
    assert_eq!(p, "/users");
}

#[test]
fn path_single_param() {
    let id = 42u32;
    let p = path!("/users/:id", id = id);
    assert_eq!(p, "/users/42");
}

#[test]
fn path_multiple_params() {
    let post_id = 1u32;
    let id = 99u32;
    let p = path!("/posts/:post_id/comments/:id", post_id = post_id, id = id);
    assert_eq!(p, "/posts/1/comments/99");
}
