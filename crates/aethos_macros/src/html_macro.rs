/// `h!` — Full HEEx-inspired HTML template proc macro.
///
/// Parses HTML-like syntax at compile time, validates tag nesting,
/// auto-escapes `{expr}` and `{@name}`, supports `:if`/`:for` shorthands
/// and `<.component />` / `<Mod.component />` invocations.
///
/// # Example
/// ```rust,ignore
/// fn greet(assigns: &Assigns) -> Html {
///     h! {
///         <div class="greeting">
///             <h1>{@title}</h1>
///             <p :if={assigns.show_sub}>Subtitle: {@subtitle}</p>
///             <ul>
///                 <li :for={item in @items}>{item}</li>
///             </ul>
///         </div>
///     }
/// }
/// ```
use proc_macro2::{Delimiter, Group, Span, TokenStream, TokenTree};
use quote::quote;
use std::collections::VecDeque;
use std::iter::FromIterator;
use syn::Result;

// ── Public entry ──────────────────────────────────────────────────────────────

pub fn expand(input: TokenStream) -> Result<TokenStream> {
    let mut cursor = Cursor::new(input);
    let nodes = parse_nodes(&mut cursor, None)?;
    let mut builder = TemplateBuilder::new();
    gen_nodes_into(&nodes, &mut builder);
    Ok(builder.finish())
}

// ── AST ───────────────────────────────────────────────────────────────────────

enum Node {
    Element(Element),
    Component(Component),
    /// `{expr}` — HTML-escaped
    Expr(TokenStream),
    /// `{raw(expr)}` — trusted, not escaped
    RawExpr(TokenStream),
    /// Literal text between tags
    Text(String),
}

struct Element {
    tag: String,
    attrs: Vec<Attr>,
    children: Vec<Node>,
    self_closing: bool,
    /// `:if={expr}` wraps the whole element
    if_cond: Option<TokenStream>,
    /// `:for={pat in expr}` wraps the whole element
    for_binding: Option<(TokenStream, TokenStream)>,
}

struct Component {
    /// `None` = `<.local_fn />`, `Some("Mod")` = `<Mod.fn />`
    module: Option<String>,
    name: String,
    attrs: Vec<Attr>,
    /// Children passed as `inner_block` slot (non-slot content between open/close tags).
    inner_block: Vec<Node>,
    /// Named slots: `<:header>...</:header>` → `("header", [nodes])`.
    named_slots: Vec<(String, Vec<Node>)>,
}

struct Attr {
    name: String,
    value: AttrValue,
}

enum AttrValue {
    Static(String),
    Dynamic(TokenStream),
    Bool,
}

// ── Cursor ────────────────────────────────────────────────────────────────────

struct Cursor {
    tokens: VecDeque<TokenTree>,
}

impl Cursor {
    fn new(ts: TokenStream) -> Self {
        Self { tokens: ts.into_iter().collect() }
    }
    fn peek(&self) -> Option<&TokenTree> { self.tokens.front() }
    fn peek2(&self) -> Option<&TokenTree> { self.tokens.get(1) }
    fn next_tok(&mut self) -> Option<TokenTree> { self.tokens.pop_front() }
    fn is_empty(&self) -> bool { self.tokens.is_empty() }

    fn is_punct(&self, c: char) -> bool {
        matches!(self.peek(), Some(TokenTree::Punct(p)) if p.as_char() == c)
    }

    fn expect_punct(&mut self, c: char) -> Result<()> {
        match self.next_tok() {
            Some(TokenTree::Punct(p)) if p.as_char() == c => Ok(()),
            other => Err(syn::Error::new(tt_span_opt(&other), format!("expected `{c}`"))),
        }
    }

    fn expect_ident(&mut self) -> Result<String> {
        match self.next_tok() {
            Some(TokenTree::Ident(i)) => Ok(i.to_string()),
            other => Err(syn::Error::new(tt_span_opt(&other), "expected identifier")),
        }
    }
}

fn tt_span_opt(tt: &Option<TokenTree>) -> Span {
    tt.as_ref().map(tt_span).unwrap_or_else(Span::call_site)
}
fn tt_span(tt: &TokenTree) -> Span {
    match tt {
        TokenTree::Ident(i) => i.span(),
        TokenTree::Punct(p) => p.span(),
        TokenTree::Literal(l) => l.span(),
        TokenTree::Group(g) => g.span(),
    }
}

// ── Parser ────────────────────────────────────────────────────────────────────

fn parse_nodes(cursor: &mut Cursor, _close_tag: Option<&str>) -> Result<Vec<Node>> {
    let mut nodes = Vec::new();
    loop {
        if cursor.is_empty() || is_close_tag(cursor) { break; }

        if cursor.is_punct('<') {
            nodes.push(parse_element_or_component(cursor)?);
            continue;
        }
        if let Some(TokenTree::Group(g)) = cursor.peek() {
            if g.delimiter() == Delimiter::Brace {
                let g = match cursor.next_tok() {
                    Some(TokenTree::Group(g)) => g,
                    _ => unreachable!(),
                };
                nodes.push(parse_brace_expr(g.stream()));
                continue;
            }
        }
        let text = collect_text(cursor);
        if !text.is_empty() { nodes.push(Node::Text(text)); }
    }
    Ok(nodes)
}

fn is_close_tag(cursor: &Cursor) -> bool {
    matches!(cursor.peek(),  Some(TokenTree::Punct(p)) if p.as_char() == '<') &&
    matches!(cursor.peek2(), Some(TokenTree::Punct(p)) if p.as_char() == '/')
}

fn collect_text(cursor: &mut Cursor) -> String {
    let mut parts = Vec::new();
    loop {
        match cursor.peek() {
            None => break,
            Some(TokenTree::Punct(p)) if p.as_char() == '<' => break,
            Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Brace => break,
            _ => parts.push(token_text(&cursor.next_tok().unwrap())),
        }
    }
    parts.join("")
}

fn token_text(t: &TokenTree) -> String {
    match t {
        TokenTree::Ident(i) => format!("{i} "),
        TokenTree::Literal(l) => {
            let s = l.to_string();
            if s.starts_with('"') && s.ends_with('"') {
                s[1..s.len()-1].to_string()
            } else {
                s
            }
        }
        TokenTree::Punct(p) => p.as_char().to_string(),
        TokenTree::Group(_) => String::new(),
    }
}

fn parse_brace_expr(ts: TokenStream) -> Node {
    let tokens: Vec<TokenTree> = ts.into_iter().collect();
    // {raw(expr)} — unescaped
    if let Some(TokenTree::Ident(i)) = tokens.first() {
        if i.to_string() == "raw" {
            if let Some(TokenTree::Group(g)) = tokens.get(1) {
                if g.delimiter() == Delimiter::Parenthesis {
                    return Node::RawExpr(g.stream());
                }
            }
        }
    }
    Node::Expr(transform_assigns(TokenStream::from_iter(tokens)))
}

fn parse_element_or_component(cursor: &mut Cursor) -> Result<Node> {
    cursor.expect_punct('<')?;

    // `<.name` — local component
    if cursor.is_punct('.') {
        cursor.next_tok();
        let name = cursor.expect_ident()?;
        let (attrs, self_closing, _, _) = parse_attrs_full(cursor)?;
        if self_closing {
            return Ok(Node::Component(Component {
                module: None, name, attrs,
                inner_block: vec![], named_slots: vec![],
            }));
        }
        let (inner_block, named_slots) = parse_component_body(cursor)?;
        consume_component_close_tag(cursor, None, &name)?;
        return Ok(Node::Component(Component {
            module: None, name, attrs, inner_block, named_slots,
        }));
    }

    if let Some(TokenTree::Ident(_)) = cursor.peek() {
        let first = cursor.expect_ident()?;
        // `<Mod.name` — module component
        if cursor.is_punct('.') {
            cursor.next_tok();
            let name = cursor.expect_ident()?;
            let (attrs, self_closing, _, _) = parse_attrs_full(cursor)?;
            if self_closing {
                return Ok(Node::Component(Component {
                    module: Some(first), name, attrs,
                    inner_block: vec![], named_slots: vec![],
                }));
            }
            let (inner_block, named_slots) = parse_component_body(cursor)?;
            consume_component_close_tag(cursor, Some(&first), &name)?;
            return Ok(Node::Component(Component {
                module: Some(first), name, attrs, inner_block, named_slots,
            }));
        }
        return parse_html_element(cursor, first);
    }

    Err(syn::Error::new(Span::call_site(), "expected tag name after `<`"))
}

/// Parse the body of a component tag: extracts `<:slot>...</:slot>` named slots
/// and any other content goes into `inner_block`. Stops at `</`.
fn parse_component_body(
    cursor: &mut Cursor,
) -> Result<(Vec<Node>, Vec<(String, Vec<Node>)>)> {
    let mut inner_block: Vec<Node> = Vec::new();
    let mut named_slots: Vec<(String, Vec<Node>)> = Vec::new();

    loop {
        if cursor.is_empty() { break; }
        // Stop at any close tag `</` (either slot close or component close)
        if is_close_tag(cursor) { break; }

        // Detect named slot open tag: `<:slot_name>`
        if cursor.is_punct('<') {
            if matches!(cursor.peek2(), Some(TokenTree::Punct(p)) if p.as_char() == ':') {
                cursor.next_tok(); // consume `<`
                cursor.next_tok(); // consume `:`
                let slot_name = cursor.expect_ident()?;
                cursor.expect_punct('>')?;
                // Parse slot content until `</:slot_name>`
                let slot_children = parse_nodes(cursor, None)?;
                // Consume `</:slot_name>`
                cursor.expect_punct('<')?;
                cursor.expect_punct('/')?;
                cursor.expect_punct(':')?;
                let close = cursor.expect_ident()?;
                if close != slot_name {
                    return Err(syn::Error::new(
                        Span::call_site(),
                        format!("mismatched slot close: expected `</:{}>`", slot_name),
                    ));
                }
                cursor.expect_punct('>')?;
                named_slots.push((slot_name, slot_children));
                continue;
            }
            // Regular HTML element or component
            inner_block.push(parse_element_or_component(cursor)?);
            continue;
        }

        if let Some(TokenTree::Group(g)) = cursor.peek() {
            if g.delimiter() == Delimiter::Brace {
                let g = match cursor.next_tok() {
                    Some(TokenTree::Group(g)) => g,
                    _ => unreachable!(),
                };
                inner_block.push(parse_brace_expr(g.stream()));
                continue;
            }
        }

        let text = collect_text(cursor);
        if !text.is_empty() { inner_block.push(Node::Text(text)); }
    }

    Ok((inner_block, named_slots))
}

/// Consume a component close tag: `</.name>` or `</Mod.name>`.
fn consume_component_close_tag(
    cursor: &mut Cursor,
    module: Option<&str>,
    name: &str,
) -> Result<()> {
    cursor.expect_punct('<')?;
    cursor.expect_punct('/')?;
    if let Some(m) = module {
        let got_mod = cursor.expect_ident()?;
        if got_mod != m {
            return Err(syn::Error::new(
                Span::call_site(),
                format!("expected `</{m}.{name}>`"),
            ));
        }
        cursor.expect_punct('.')?;
    } else {
        cursor.expect_punct('.')?;
    }
    let got_name = cursor.expect_ident()?;
    if got_name != name {
        return Err(syn::Error::new(
            Span::call_site(),
            format!("expected closing tag for component `{name}`"),
        ));
    }
    cursor.expect_punct('>')
}

fn parse_html_element(cursor: &mut Cursor, tag: String) -> Result<Node> {
    let (attrs, self_closing, if_cond, for_binding) = parse_attrs_full(cursor)?;
    if self_closing {
        return Ok(Node::Element(Element {
            tag, attrs, children: vec![], self_closing: true, if_cond, for_binding,
        }));
    }
    let children = parse_nodes(cursor, Some(&tag))?;
    consume_close_tag(cursor, &tag)?;
    Ok(Node::Element(Element {
        tag, attrs, children, self_closing: false, if_cond, for_binding,
    }))
}

fn parse_attrs_full(
    cursor: &mut Cursor,
) -> Result<(Vec<Attr>, bool, Option<TokenStream>, Option<(TokenStream, TokenStream)>)> {
    let mut attrs = Vec::new();
    let mut if_cond = None;
    let mut for_binding = None;
    loop {
        if cursor.is_punct('/') {
            cursor.next_tok();
            cursor.expect_punct('>')?;
            return Ok((attrs, true, if_cond, for_binding));
        }
        if cursor.is_punct('>') {
            cursor.next_tok();
            return Ok((attrs, false, if_cond, for_binding));
        }
        // `:if` / `:for`
        if cursor.is_punct(':') {
            cursor.next_tok();
            let key = cursor.expect_ident()?;
            cursor.expect_punct('=')?;
            let g = match cursor.next_tok() {
                Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Brace => g,
                other => return Err(syn::Error::new(tt_span_opt(&other), "expected `{...}`")),
            };
            match key.as_str() {
                "if"  => if_cond     = Some(transform_assigns(g.stream())),
                "for" => for_binding = Some(parse_for_binding(g.stream())?),
                _     => {}
            }
            continue;
        }
        // Regular attribute name (may be hyphenated: phx-click, data-value, etc.)
        let name = match cursor.next_tok() {
            Some(TokenTree::Ident(i)) => {
                let mut s = i.to_string();
                // Collect trailing `-ident` segments for hyphenated names
                while cursor.is_punct('-') {
                    if let Some(TokenTree::Ident(_)) = cursor.peek2() {
                        cursor.next_tok(); // consume `-`
                        s.push('-');
                        s.push_str(&cursor.expect_ident().unwrap_or_default());
                    } else {
                        break;
                    }
                }
                s
            }
            Some(TokenTree::Literal(l)) => l.to_string().trim_matches('"').to_owned(),
            other => return Err(syn::Error::new(tt_span_opt(&other), "expected attribute name")),
        };
        if cursor.is_punct('=') {
            cursor.next_tok();
            attrs.push(Attr { name, value: parse_attr_value(cursor)? });
        } else {
            attrs.push(Attr { name, value: AttrValue::Bool });
        }
    }
}

fn parse_attrs(cursor: &mut Cursor) -> Result<Vec<Attr>> {
    let (attrs, ..) = parse_attrs_full(cursor)?;
    Ok(attrs)
}

fn parse_attr_value(cursor: &mut Cursor) -> Result<AttrValue> {
    match cursor.next_tok() {
        Some(TokenTree::Literal(l)) => {
            let s = l.to_string();
            Ok(AttrValue::Static(s.trim_matches('"').to_owned()))
        }
        Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Brace => {
            Ok(AttrValue::Dynamic(transform_assigns(g.stream())))
        }
        other => Err(syn::Error::new(tt_span_opt(&other), "expected attribute value (`\"str\"` or `{expr}`)"))
    }
}

fn parse_for_binding(ts: TokenStream) -> Result<(TokenStream, TokenStream)> {
    let toks: Vec<TokenTree> = ts.into_iter().collect();
    let pos = toks.iter().position(|t| matches!(t, TokenTree::Ident(i) if i.to_string() == "in"));
    match pos {
        Some(p) => Ok((
            transform_assigns(toks[..p].iter().cloned().collect()),
            transform_assigns(toks[p+1..].iter().cloned().collect()),
        )),
        None => Err(syn::Error::new(Span::call_site(), "`:for` expects `pattern in expr`")),
    }
}

fn consume_close_tag(cursor: &mut Cursor, tag: &str) -> Result<()> {
    cursor.expect_punct('<')?;
    cursor.expect_punct('/')?;
    let close = cursor.expect_ident()?;
    if close != tag {
        return Err(syn::Error::new(
            Span::call_site(),
            format!("mismatched closing tag: expected `</{tag}>`, found `</{close}>`"),
        ));
    }
    cursor.expect_punct('>')
}

/// Replace `@ident` → `assigns.ident` throughout a token stream.
fn transform_assigns(ts: TokenStream) -> TokenStream {
    let toks: Vec<TokenTree> = ts.into_iter().collect();
    let mut out: Vec<TokenTree> = Vec::with_capacity(toks.len());
    let mut i = 0;
    while i < toks.len() {
        if let TokenTree::Punct(p) = &toks[i] {
            if p.as_char() == '@' {
                if let Some(TokenTree::Ident(name)) = toks.get(i + 1) {
                    let sp = p.span();
                    let a  = proc_macro2::Ident::new("assigns", sp);
                    let mut dot = proc_macro2::Punct::new('.', proc_macro2::Spacing::Alone);
                    dot.set_span(sp);
                    let f  = proc_macro2::Ident::new(&name.to_string(), name.span());
                    out.extend([TokenTree::Ident(a), TokenTree::Punct(dot), TokenTree::Ident(f)]);
                    i += 2;
                    continue;
                }
            }
        }
        if let TokenTree::Group(g) = &toks[i] {
            let inner = transform_assigns(g.stream());
            out.push(TokenTree::Group(Group::new(g.delimiter(), inner)));
            i += 1;
            continue;
        }
        out.push(toks[i].clone());
        i += 1;
    }
    TokenStream::from_iter(out)
}

// ── Code generation ───────────────────────────────────────────────────────────
//
// Templates are represented as statics (compile-time string fragments) + dynamics
// (runtime expression values). The h! macro collects these into a TemplateBuilder
// which emits code that constructs an `aethos::Template` at runtime.
//
// Rules:
//   • Static text / static attrs  → accumulated into the current static buffer
//   • Dynamic expressions         → flush static buffer, add a new dynamic slot
//   • :if / :for / components     → rendered to a String, single dynamic slot
// ─────────────────────────────────────────────────────────────────────────────

struct TemplateBuilder {
    /// Finalized static fragments (one per `push_dynamic` call, plus one at the end).
    statics: Vec<String>,
    /// Runtime dynamic expression code snippets, each produces a `String`.
    dynamics: Vec<TokenStream>,
    /// Pending static text not yet flushed.
    current_static: String,
}

impl TemplateBuilder {
    fn new() -> Self {
        Self { statics: vec![], dynamics: vec![], current_static: String::new() }
    }

    fn push_static(&mut self, s: &str) {
        self.current_static.push_str(s);
    }

    /// Flush pending static, record a new dynamic slot.
    /// `code` must be a TokenStream that evaluates to `String`.
    fn push_dynamic(&mut self, code: TokenStream) {
        let s = std::mem::take(&mut self.current_static);
        self.statics.push(s);
        self.dynamics.push(code);
    }

    /// Generate code that builds an `aethos::Template`.
    fn finish(mut self) -> TokenStream {
        self.statics.push(self.current_static);
        let statics: Vec<_> = self.statics.iter().map(|s| s.as_str()).collect();
        let dynamics = &self.dynamics;
        quote! {
            ::aethos::Template {
                statics: vec![#(#statics),*],
                dynamics: vec![#(#dynamics),*],
            }
        }
    }

    /// Generate code that builds a `String` (for embedding as a dynamic slot).
    fn finish_string(mut self) -> TokenStream {
        self.statics.push(self.current_static);
        let statics: Vec<_> = self.statics.iter().map(|s| s.as_str()).collect();
        let dynamics = &self.dynamics;
        if dynamics.is_empty() {
            // Pure static — emit a string literal
            let combined: String = self.statics.join("");
            return quote! { ::std::string::String::from(#combined) };
        }
        let mut pieces: Vec<TokenStream> = Vec::new();
        for (i, s) in statics.iter().enumerate() {
            pieces.push(quote! { __sb.push_str(#s); });
            if i < dynamics.len() {
                let d = &dynamics[i];
                pieces.push(quote! { __sb.push_str(&#d); });
            }
        }
        quote! {
            {
                let mut __sb = ::std::string::String::new();
                #(#pieces)*
                __sb
            }
        }
    }
}

fn gen_nodes_into(nodes: &[Node], b: &mut TemplateBuilder) {
    for node in nodes { gen_node_into(node, b); }
}

fn gen_node_into(node: &Node, b: &mut TemplateBuilder) {
    match node {
        Node::Text(s) => b.push_static(&escape_static(s)),
        Node::Expr(ts) => b.push_dynamic(quote! {
            ::aethos::html_escape(&::std::format!("{}", #ts))
        }),
        Node::RawExpr(ts) => b.push_dynamic(quote! {
            ::std::format!("{}", #ts)
        }),
        Node::Element(e)   => gen_element_into(e, b),
        Node::Component(c) => gen_component_into(c, b),
    }
}

fn gen_element_into(e: &Element, b: &mut TemplateBuilder) {
    // :for — render inner element to string, one dynamic slot
    if let Some((pat, expr)) = &e.for_binding {
        let mut inner = TemplateBuilder::new();
        gen_element_bare_into(e, &mut inner);
        let inner_str = inner.finish_string();
        b.push_dynamic(quote! {
            { let mut __acc = ::std::string::String::new(); for #pat in #expr { __acc.push_str(&#inner_str); } __acc }
        });
        return;
    }
    // :if — render inner element to string, one dynamic slot
    if let Some(cond) = &e.if_cond {
        let mut inner = TemplateBuilder::new();
        gen_element_bare_into(e, &mut inner);
        let inner_str = inner.finish_string();
        b.push_dynamic(quote! {
            if #cond { #inner_str } else { ::std::string::String::new() }
        });
        return;
    }
    gen_element_bare_into(e, b);
}

/// Generate element HTML without processing :if/:for (used when wrapping them above).
fn gen_element_bare_into(e: &Element, b: &mut TemplateBuilder) {
    let tag = &e.tag;
    b.push_static(&format!("<{tag}"));
    for attr in &e.attrs { gen_attr_into(attr, b); }
    if e.self_closing {
        b.push_static(" />");
    } else {
        b.push_static(">");
        gen_nodes_into(&e.children, b);
        b.push_static(&format!("</{tag}>"));
    }
}

fn gen_attr_into(a: &Attr, b: &mut TemplateBuilder) {
    let name = &a.name;
    match &a.value {
        AttrValue::Bool        => b.push_static(&format!(" {name}")),
        AttrValue::Static(s)   => b.push_static(&format!(" {name}=\"{s}\"")),
        AttrValue::Dynamic(ts) => {
            b.push_static(&format!(" {name}=\""));
            b.push_dynamic(quote! {
                ::aethos::html_escape(&::std::format!("{}", #ts))
            });
            b.push_static("\"");
        }
    }
}

fn gen_component_into(c: &Component, b: &mut TemplateBuilder) {
    let n  = proc_macro2::Ident::new(&c.name, Span::call_site());
    let attr_calls: Vec<TokenStream> = c.attrs.iter().map(|a| match &a.value {
        AttrValue::Dynamic(ts) => quote! { .put(#ts) },
        AttrValue::Static(s)   => quote! { .put(#s.to_string()) },
        AttrValue::Bool        => quote! {},
    }).collect();

    // Named slots — rendered to Html (pre-rendered string)
    let slot_calls: Vec<TokenStream> = c.named_slots.iter().map(|(name, children)| {
        let mut sb = TemplateBuilder::new();
        gen_nodes_into(children, &mut sb);
        let html_str = sb.finish_string();
        quote! { .put_slot(#name, ::aethos::Html(#html_str)) }
    }).collect();

    // inner_block — non-slot children
    let inner_block_call = if !c.inner_block.is_empty() {
        let mut ib = TemplateBuilder::new();
        gen_nodes_into(&c.inner_block, &mut ib);
        let html_str = ib.finish_string();
        quote! { .put_slot("inner_block", ::aethos::Html(#html_str)) }
    } else {
        quote! {}
    };

    let call = if let Some(m) = &c.module {
        let m = proc_macro2::Ident::new(m, Span::call_site());
        quote! { #m::#n }
    } else {
        quote! { #n }
    };

    // Component returns Template; render to String for embedding as a dynamic slot.
    b.push_dynamic(quote! {
        {
            let __ca = ::aethos::Assigns::new()#(#attr_calls)*#(#slot_calls)*#inner_block_call;
            #call(&__ca).render_string()
        }
    });
}

/// Escape a static string literal at macro-expansion time.
fn escape_static(s: &str) -> String {
    s.chars().flat_map(|c| match c {
        '&'  => "&amp;".chars().collect::<Vec<_>>(),
        '<'  => "&lt;".chars().collect(),
        '>'  => "&gt;".chars().collect(),
        '"'  => "&quot;".chars().collect(),
        '\'' => "&#39;".chars().collect(),
        _    => vec![c],
    }).collect()
}

