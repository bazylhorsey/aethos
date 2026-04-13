use aethos::{h, Template};

// ── Template::diff_pairs ─────────────────────────────────────────────────────

#[test]
fn diff_no_dynamics_always_same() {
    let a = h! { <p>static text</p> };
    let b = h! { <p>static text</p> };
    // Both have 0 dynamics; diff should be Some([]) — no changed slots
    let pairs = a.diff_pairs(&b).expect("same structure → Some");
    assert!(pairs.is_empty(), "no dynamics changed");
}

#[test]
fn diff_returns_only_changed_slot() {
    let v1 = String::from("Alice");
    let t1 = h! { <p>{v1}</p> };

    let v2 = String::from("Bob");
    let t2 = h! { <p>{v2}</p> };

    let pairs = t2.diff_pairs(&t1).expect("same structure");
    assert_eq!(pairs.len(), 1, "one slot changed");
    assert_eq!(pairs[0].0, 0, "slot index 0");
    assert!(pairs[0].1.contains("Bob"), "slot value is Bob");
}

#[test]
fn diff_unchanged_returns_empty() {
    let name = String::from("Alice");
    let t1 = h! { <p>{name}</p> };
    let name = String::from("Alice");
    let t2 = h! { <p>{name}</p> };

    let pairs = t2.diff_pairs(&t1).expect("same structure");
    assert!(pairs.is_empty(), "nothing changed");
    assert!(t2.is_same_as(&t1));
}

#[test]
fn diff_structural_change_returns_none() {
    // Same element but different number of dynamic slots → structural change
    let show = true;
    let t1 = h! { <div>{show.to_string()}<span>extra</span></div> };
    let show = false;
    let t2 = h! { <div :if={show}><span>extra</span></div> };

    // t1 has 1 dynamic slot; t2 likely has 1 dynamic slot from :if
    // But regardless, diff_pairs returns None only when lengths differ.
    // This test verifies that a real structural difference is detected.
    // We construct two templates with different dynamic lengths manually:
    let big = Template {
        statics: vec!["<div>", "<span>", "</span></div>"],
        dynamics: vec!["A".into(), "B".into()],
    };
    let small = Template {
        statics: vec!["<div>", "</div>"],
        dynamics: vec!["A".into()],
    };
    assert!(big.diff_pairs(&small).is_none(), "different lengths → None");
}

#[test]
fn diff_multiple_slots_partial_change() {
    let a = String::from("Alice");
    let n = 42u32;
    let t1 = h! { <p>{a}{n.to_string()}</p> };

    let a = String::from("Alice");   // same
    let n = 99u32;                   // changed
    let t2 = h! { <p>{a}{n.to_string()}</p> };

    let pairs = t2.diff_pairs(&t1).expect("same structure");
    // Only slot 1 (n) changed; slot 0 (a) is the same
    assert_eq!(pairs.len(), 1, "only one slot changed");
    let (idx, _val) = pairs[0];
    assert_eq!(idx, 1, "slot index 1 changed");
}

// ── is_same_as ───────────────────────────────────────────────────────────────

#[test]
fn is_same_as_identical() {
    let v = String::from("hello");
    let t1 = h! { <span>{v}</span> };
    let v = String::from("hello");
    let t2 = h! { <span>{v}</span> };
    assert!(t2.is_same_as(&t1));
}

#[test]
fn is_same_as_different() {
    let v = String::from("hello");
    let t1 = h! { <span>{v}</span> };
    let v = String::from("world");
    let t2 = h! { <span>{v}</span> };
    assert!(!t2.is_same_as(&t1));
}

// ── render_string ────────────────────────────────────────────────────────────

#[test]
fn render_string_interleaves_correctly() {
    let t = Template {
        statics:  vec!["<p>", " and ", "</p>"],
        dynamics: vec!["hello".into(), "world".into()],
    };
    assert_eq!(t.render_string(), "<p>hello and world</p>");
}

#[test]
fn render_string_no_dynamics() {
    let t = Template {
        statics:  vec!["<hr/>"],
        dynamics: vec![],
    };
    assert_eq!(t.render_string(), "<hr/>");
}
