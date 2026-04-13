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
    let stmts = gen_nodes(&nodes);
    Ok(quote! {
        {
            let mut __html = ::std::string::String::new();
            #stmts
            ::aethos_html::Html(__html)
        }
    })
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
        let attrs = parse_attrs(cursor)?;
        return Ok(Node::Component(Component { module: None, name, attrs }));
    }

    if let Some(TokenTree::Ident(_)) = cursor.peek() {
        let first = cursor.expect_ident()?;
        // `<Mod.name` — module component
        if cursor.is_punct('.') {
            cursor.next_tok();
            let name = cursor.expect_ident()?;
            let attrs = parse_attrs(cursor)?;
            return Ok(Node::Component(Component { module: Some(first), name, attrs }));
        }
        return parse_html_element(cursor, first);
    }

    Err(syn::Error::new(Span::call_site(), "expected tag name after `<`"))
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
        // Regular attribute name
        let name = match cursor.next_tok() {
            Some(TokenTree::Ident(i)) => i.to_string(),
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

fn gen_nodes(nodes: &[Node]) -> TokenStream {
    let stmts: Vec<TokenStream> = nodes.iter().map(gen_node).collect();
    quote! { #(#stmts)* }
}

fn gen_node(node: &Node) -> TokenStream {
    match node {
        Node::Text(s) => {
            let e = escape_static(s);
            quote! { __html.push_str(#e); }
        }
        Node::Expr(ts) => quote! {
            __html.push_str(&::aethos_html::html_escape(&format!("{}", #ts)));
        },
        Node::RawExpr(ts) => quote! {
            __html.push_str(&format!("{}", #ts));
        },
        Node::Component(c) => gen_component(c),
        Node::Element(e)   => gen_element(e),
    }
}

fn gen_element(e: &Element) -> TokenStream {
    let inner = gen_element_inner(e);
    let wrapped = if let Some((pat, expr)) = &e.for_binding {
        quote! { for #pat in #expr { #inner } }
    } else { inner };
    if let Some(cond) = &e.if_cond {
        quote! { if #cond { #wrapped } }
    } else { wrapped }
}

fn gen_element_inner(e: &Element) -> TokenStream {
    let tag = &e.tag;
    let open = format!("<{tag}");
    let attr_ts: Vec<TokenStream> = e.attrs.iter().map(gen_attr).collect();
    let children = gen_nodes(&e.children);
    if e.self_closing {
        quote! {
            __html.push_str(#open);
            #(#attr_ts)*
            __html.push_str(" />");
        }
    } else {
        let close = format!("</{tag}>");
        quote! {
            __html.push_str(#open);
            #(#attr_ts)*
            __html.push_str(">");
            #children
            __html.push_str(#close);
        }
    }
}

fn gen_attr(a: &Attr) -> TokenStream {
    let name = &a.name;
    match &a.value {
        AttrValue::Bool => quote! {
            __html.push_str(concat!(" ", #name));
        },
        AttrValue::Static(s) => {
            let r = format!(" {name}=\"{s}\"");
            quote! { __html.push_str(#r); }
        }
        AttrValue::Dynamic(ts) => quote! {
            __html.push_str(&format!(
                " {}=\"{}\"",
                #name,
                ::aethos_html::html_escape(&format!("{}", #ts))
            ));
        },
    }
}

fn gen_component(c: &Component) -> TokenStream {
    let n = proc_macro2::Ident::new(&c.name, Span::call_site());
    let attr_calls: Vec<TokenStream> = c.attrs.iter().map(|a| match &a.value {
        AttrValue::Dynamic(ts) => quote! { .put(#ts) },
        AttrValue::Static(s)   => quote! { .put(#s.to_string()) },
        AttrValue::Bool        => quote! {},
    }).collect();
    let call = if let Some(m) = &c.module {
        let m = proc_macro2::Ident::new(m, Span::call_site());
        quote! { #m::#n }
    } else {
        quote! { #n }
    };
    quote! {
        {
            let __ca = ::aethos_html::Assigns::new()#(#attr_calls)*;
            __html.push_str(&#call(__ca).0);
        }
    }
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
