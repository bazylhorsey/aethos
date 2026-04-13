use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{
    braced, bracketed, parenthesized,
    parse::{Parse, ParseStream},
    token, Ident, LitStr, Path, Result, Token,
};

// ── AST nodes ────────────────────────────────────────────────────────────────

struct RouterDef {
    items: Vec<RouterItem>,
}

enum RouterItem {
    Pipeline(PipelineDef),
    Scope(ScopeDef),
}

struct PipelineDef {
    name: Ident,
    plugs: Vec<PlugCall>,
}

struct PlugCall {
    plug_type: Path,
    args: Option<TokenStream>,
}

struct ScopeDef {
    prefix: LitStr,
    items: Vec<ScopeItem>,
}

enum ScopeItem {
    PipeThrough(Vec<Ident>),
    Route(RouteDef),
    LiveRoute(LiveRouteDef),
    WebSocketRoute(WebSocketRouteDef),
    Resources(ResourcesDef),
    NestedScope(ScopeDef),
}

struct RouteDef {
    method: Ident,
    path: LitStr,
    controller: Path,
    action: Ident,
}

struct LiveRouteDef {
    path: LitStr,
    live_view: Path,
}

struct WebSocketRouteDef {
    path: LitStr,
    socket: Path,
}

struct ResourcesDef {
    path: LitStr,
    controller: Path,
}

// ── Parsing ───────────────────────────────────────────────────────────────────

impl Parse for RouterDef {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut items = Vec::new();
        while !input.is_empty() {
            items.push(input.parse()?);
        }
        Ok(RouterDef { items })
    }
}

impl Parse for RouterItem {
    fn parse(input: ParseStream) -> Result<Self> {
        let lookahead = input.lookahead1();
        if lookahead.peek(kw::pipeline) {
            input.parse::<kw::pipeline>()?;
            let name: Ident = if input.peek(Token![:]) {
                input.parse::<Token![:]>()?;
                input.parse()?
            } else {
                input.parse()?
            };
            let content;
            braced!(content in input);
            let mut plugs = Vec::new();
            while !content.is_empty() {
                plugs.push(content.parse::<PlugCall>()?);
            }
            Ok(RouterItem::Pipeline(PipelineDef { name, plugs }))
        } else if lookahead.peek(kw::scope) {
            Ok(RouterItem::Scope(input.parse()?))
        } else {
            Err(lookahead.error())
        }
    }
}

impl Parse for PlugCall {
    fn parse(input: ParseStream) -> Result<Self> {
        // plug!(TypePath) or plug!(TypePath, arg1, arg2)
        input.parse::<kw::plug>()?;
        input.parse::<Token![!]>()?;
        let content;
        parenthesized!(content in input);
        let plug_type: Path = content.parse()?;
        let args = if content.peek(Token![,]) {
            content.parse::<Token![,]>()?;
            let ts: TokenStream = content.parse()?;
            Some(ts)
        } else {
            None
        };
        // optional trailing semicolon
        let _ = input.parse::<Token![;]>();
        Ok(PlugCall { plug_type, args })
    }
}

impl Parse for ScopeDef {
    fn parse(input: ParseStream) -> Result<Self> {
        input.parse::<kw::scope>()?;
        let prefix: LitStr = input.parse()?;
        let content;
        braced!(content in input);
        let mut items = Vec::new();
        while !content.is_empty() {
            items.push(content.parse::<ScopeItem>()?);
        }
        Ok(ScopeDef { prefix, items })
    }
}

impl Parse for ScopeItem {
    fn parse(input: ParseStream) -> Result<Self> {
        // pipe_through!(:name1, :name2)  or  pipe_through!([:name1])
        if input.peek(kw::pipe_through) {
            input.parse::<kw::pipe_through>()?;
            input.parse::<Token![!]>()?;
            let content;
            parenthesized!(content in input);
            let _ = input.parse::<Token![;]>().ok();
            // accept :name or [:name, :name]
            let names = parse_pipeline_names(&content)?;
            let _ = input.parse::<Token![;]>().ok();
            return Ok(ScopeItem::PipeThrough(names));
        }
        // live!("/path", LiveView)
        if input.peek(kw::live) {
            input.parse::<kw::live>()?;
            input.parse::<Token![!]>()?;
            let content;
            parenthesized!(content in input);
            let path: LitStr = content.parse()?;
            content.parse::<Token![,]>()?;
            let live_view: Path = content.parse()?;
            let _ = input.parse::<Token![;]>().ok();
            return Ok(ScopeItem::LiveRoute(LiveRouteDef { path, live_view }));
        }
        // websocket!("/path", Socket)
        if input.peek(kw::websocket) {
            input.parse::<kw::websocket>()?;
            input.parse::<Token![!]>()?;
            let content;
            parenthesized!(content in input);
            let path: LitStr = content.parse()?;
            content.parse::<Token![,]>()?;
            let socket: Path = content.parse()?;
            let _ = input.parse::<Token![;]>().ok();
            return Ok(ScopeItem::WebSocketRoute(WebSocketRouteDef { path, socket }));
        }
        // resources!("/path", Controller)
        if input.peek(kw::resources) {
            input.parse::<kw::resources>()?;
            input.parse::<Token![!]>()?;
            let content;
            parenthesized!(content in input);
            let path: LitStr = content.parse()?;
            content.parse::<Token![,]>()?;
            let controller: Path = content.parse()?;
            let _ = input.parse::<Token![;]>().ok();
            return Ok(ScopeItem::Resources(ResourcesDef { path, controller }));
        }
        // nested scope
        if input.peek(kw::scope) {
            return Ok(ScopeItem::NestedScope(input.parse()?));
        }
        // get!("/path", Controller, action) etc.
        let method: Ident = input.parse()?;
        input.parse::<Token![!]>()?;
        let content;
        parenthesized!(content in input);
        let path: LitStr = content.parse()?;
        content.parse::<Token![,]>()?;
        let controller: Path = content.parse()?;
        content.parse::<Token![,]>()?;
        let action: Ident = content.parse()?;
        let _ = input.parse::<Token![;]>().ok();
        Ok(ScopeItem::Route(RouteDef { method, path, controller, action }))
    }
}

fn parse_pipeline_names(input: ParseStream) -> Result<Vec<Ident>> {
    let mut names = Vec::new();
    // handle both :name and [:name, ...]
    if input.peek(token::Bracket) {
        let content;
        bracketed!(content in input);
        loop {
            content.parse::<Token![:]>()?;
            names.push(content.parse::<Ident>()?);
            if content.is_empty() {
                break;
            }
            content.parse::<Token![,]>()?;
        }
    } else {
        loop {
            input.parse::<Token![:]>()?;
            names.push(input.parse::<Ident>()?);
            if !input.peek(Token![,]) {
                break;
            }
            input.parse::<Token![,]>()?;
        }
    }
    Ok(names)
}

mod kw {
    syn::custom_keyword!(pipeline);
    syn::custom_keyword!(scope);
    syn::custom_keyword!(pipe_through);
    syn::custom_keyword!(plug);
    syn::custom_keyword!(live);
    syn::custom_keyword!(websocket);
    syn::custom_keyword!(resources);
}

// ── Code generation ───────────────────────────────────────────────────────────

pub fn expand(input: TokenStream) -> Result<TokenStream> {
    let def: RouterDef = syn::parse2(input)?;

    // Separate pipelines and scopes
    let mut pipeline_defs: Vec<&PipelineDef> = Vec::new();
    let mut scope_defs: Vec<&ScopeDef> = Vec::new();

    for item in &def.items {
        match item {
            RouterItem::Pipeline(p) => pipeline_defs.push(p),
            RouterItem::Scope(s) => scope_defs.push(s),
        }
    }

    // Generate pipeline run functions (named pipeline_<name>)
    let pipeline_fns: Vec<TokenStream> = pipeline_defs
        .iter()
        .map(|p| gen_pipeline_fn(p))
        .collect();

    // Generate the router
    let route_blocks: Vec<TokenStream> = scope_defs
        .iter()
        .map(|s| gen_scope(s, &pipeline_defs))
        .collect();

    Ok(quote! {
        {
            #(#pipeline_fns)*

            let mut __router = ::axum::Router::new();
            #(#route_blocks)*
            __router
        }
    })
}

fn gen_pipeline_fn(p: &PipelineDef) -> TokenStream {
    let fn_name = format_ident!("__pipeline_{}", p.name);
    let plug_applications: Vec<TokenStream> = p
        .plugs
        .iter()
        .map(|plug| {
            let ty = &plug.plug_type;
            if let Some(args) = &plug.args {
                quote! {
                    conn = ::aethos_core::Plug::call(&#ty::init((#args,)), conn,
                        ::aethos_core::Next::terminal()).await;
                    if conn.halted { return conn; }
                }
            } else {
                quote! {
                    conn = ::aethos_core::Plug::call(&#ty::default(), conn,
                        ::aethos_core::Next::terminal()).await;
                    if conn.halted { return conn; }
                }
            }
        })
        .collect();

    quote! {
        async fn #fn_name(mut conn: ::aethos_core::Conn) -> ::aethos_core::Conn {
            #(#plug_applications)*
            conn
        }
    }
}

fn gen_scope(scope: &ScopeDef, pipelines: &[&PipelineDef]) -> TokenStream {
    let prefix = &scope.prefix;
    let mut route_tokens: Vec<TokenStream> = Vec::new();
    let mut pipe_names: Vec<Ident> = Vec::new();

    for item in &scope.items {
        match item {
            ScopeItem::PipeThrough(names) => {
                pipe_names.extend(names.iter().cloned());
            }
            ScopeItem::Route(r) => {
                route_tokens.push(gen_route(r, prefix, &pipe_names));
            }
            ScopeItem::LiveRoute(lr) => {
                route_tokens.push(gen_live_route(lr, prefix, &pipe_names));
            }
            ScopeItem::WebSocketRoute(ws) => {
                route_tokens.push(gen_websocket_route(ws, prefix, &pipe_names));
            }
            ScopeItem::Resources(res) => {
                route_tokens.push(gen_resources(res, prefix, &pipe_names));
            }
            ScopeItem::NestedScope(ns) => {
                route_tokens.push(gen_scope(ns, pipelines));
            }
        }
    }

    quote! {
        #(#route_tokens)*
    }
}

fn pipeline_runner_calls(pipe_names: &[Ident]) -> TokenStream {
    let calls: Vec<TokenStream> = pipe_names
        .iter()
        .map(|n| {
            let fn_name = format_ident!("__pipeline_{}", n);
            quote! {
                let conn = #fn_name(conn).await;
                if conn.halted { return conn.into_response(); }
            }
        })
        .collect();
    quote! { #(#calls)* }
}

fn gen_route(r: &RouteDef, prefix: &LitStr, pipe_names: &[Ident]) -> TokenStream {
    let method = &r.method;
    let path = concat_path(prefix, &r.path);
    let controller = &r.controller;
    let action = &r.action;
    let pipeline_calls = pipeline_runner_calls(pipe_names);

    let axum_method = match method.to_string().to_lowercase().as_str() {
        "get" => quote! { ::axum::routing::get },
        "post" => quote! { ::axum::routing::post },
        "put" => quote! { ::axum::routing::put },
        "patch" => quote! { ::axum::routing::patch },
        "delete" => quote! { ::axum::routing::delete },
        "head" => quote! { ::axum::routing::head },
        "options" => quote! { ::axum::routing::options },
        _ => quote! { ::axum::routing::any },
    };

    quote! {
        __router = __router.route(#path, #axum_method(|req: ::axum::extract::Request| async move {
            let conn = ::aethos_core::Conn::new(req);
            #pipeline_calls
            let conn = #controller::#action(conn).await;
            conn.into_response()
        }));
    }
}

fn gen_live_route(r: &LiveRouteDef, prefix: &LitStr, pipe_names: &[Ident]) -> TokenStream {
    let path = concat_path(prefix, &r.path);
    let live_view = &r.live_view;
    let pipeline_calls = pipeline_runner_calls(pipe_names);

    quote! {
        __router = __router.route(#path, ::axum::routing::get(|req: ::axum::extract::Request| async move {
            let conn = ::aethos_core::Conn::new(req);
            #pipeline_calls
            ::aethos_live::LiveView::handle_request(&#live_view, conn).await
        }));
    }
}

fn gen_websocket_route(r: &WebSocketRouteDef, prefix: &LitStr, _pipe_names: &[Ident]) -> TokenStream {
    let path = concat_path(prefix, &r.path);
    let socket = &r.socket;

    quote! {
        __router = __router.route(#path, ::axum::routing::get(|
            ws: ::axum::extract::WebSocketUpgrade,
            req: ::axum::extract::Request,
        | async move {
            ws.on_upgrade(|socket| ::aethos_channels::handle_socket(#socket::new(), socket))
        }));
    }
}

fn gen_resources(r: &ResourcesDef, prefix: &LitStr, pipe_names: &[Ident]) -> TokenStream {
    let base = r.path.value();
    let prefix_val = prefix.value().trim_end_matches('/').to_owned();
    let span = r.path.span();
    let controller = &r.controller;
    let pipeline_calls = pipeline_runner_calls(pipe_names);
    let pc2 = pipeline_calls.clone();
    let pc3 = pipeline_calls.clone();
    let pc4 = pipeline_calls.clone();
    let pc5 = pipeline_calls.clone();
    let pc6 = pipeline_calls.clone();
    let pc7 = pipeline_calls.clone();

    let index_path = LitStr::new(&format!("{}{}", prefix_val, base), span);
    let new_path = LitStr::new(&format!("{}{}/new", prefix_val, base), span);
    let create_path = LitStr::new(&format!("{}{}", prefix_val, base), span);
    let show_path = LitStr::new(&format!("{}{}/:id", prefix_val, base), span);
    let edit_path = LitStr::new(&format!("{}{}/:id/edit", prefix_val, base), span);
    let update_path = LitStr::new(&format!("{}{}/:id", prefix_val, base), span);
    let delete_path = LitStr::new(&format!("{}{}/:id", prefix_val, base), span);

    quote! {
        __router = __router
            .route(#index_path, ::axum::routing::get(|req: ::axum::extract::Request| async move {
                let conn = ::aethos_core::Conn::new(req);
                #pipeline_calls
                let conn = #controller::index(conn).await;
                conn.into_response()
            }))
            .route(#new_path, ::axum::routing::get(|req: ::axum::extract::Request| async move {
                let conn = ::aethos_core::Conn::new(req);
                #pc2
                let conn = #controller::new(conn).await;
                conn.into_response()
            }))
            .route(#create_path, ::axum::routing::post(|req: ::axum::extract::Request| async move {
                let conn = ::aethos_core::Conn::new(req);
                #pc3
                let conn = #controller::create(conn).await;
                conn.into_response()
            }))
            .route(#show_path, ::axum::routing::get(|req: ::axum::extract::Request| async move {
                let conn = ::aethos_core::Conn::new(req);
                #pc4
                let conn = #controller::show(conn).await;
                conn.into_response()
            }))
            .route(#edit_path, ::axum::routing::get(|req: ::axum::extract::Request| async move {
                let conn = ::aethos_core::Conn::new(req);
                #pc5
                let conn = #controller::edit(conn).await;
                conn.into_response()
            }))
            .route(#update_path, ::axum::routing::put(|req: ::axum::extract::Request| async move {
                let conn = ::aethos_core::Conn::new(req);
                #pc6
                let conn = #controller::update(conn).await;
                conn.into_response()
            }))
            .route(#delete_path, ::axum::routing::delete(|req: ::axum::extract::Request| async move {
                let conn = ::aethos_core::Conn::new(req);
                #pc7
                let conn = #controller::delete(conn).await;
                conn.into_response()
            }));
    }
}

fn concat_path(prefix: &LitStr, path: &LitStr) -> LitStr {
    let p = prefix.value();
    let p = p.trim_end_matches('/');
    let r = path.value();
    let r = r.trim_start_matches('/');
    let full = if r.is_empty() {
        format!("{}/", p)
    } else {
        format!("{}/{}", p, r)
    };
    // Axum uses `:param` style which matches Phoenix's `:param`
    LitStr::new(&full, prefix.span())
}
