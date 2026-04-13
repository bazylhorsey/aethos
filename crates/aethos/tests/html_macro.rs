use aethos::h;

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
