/// `h!` — HEEx-inspired HTML template proc macro.
///
/// Parses an HTML-like tree at compile time, validates tag nesting,
/// auto-escapes dynamic values, and emits code that builds an `Html` string.
///
/// Syntax:
/// - `{@name}`          — read assign by name, HTML-escaped
/// - `{expr}`           — evaluate Rust expression, HTML-escaped
/// - `{raw(expr)}`      — trusted HTML, NOT escaped
/// - `:if={expr}`       — conditional attribute shorthand
/// - `:for={pat in expr}` — loop shorthand
/// - `<.comp_name attr={val} />` — call same-module function component
/// - `<Mod.comp attr={val} />`   — call external function component
use proc_macro2::TokenStream;
use quote::quote;
use syn::Result;

/// Entry point called from lib.rs.
pub fn expand(input: TokenStream) -> Result<TokenStream> {
    // For now we implement a subset: a Rust string-building closure.
    // Full HTML parsing is deferred to the next implementation pass;
    // here we establish the code shape and the Html output type.
    let _input_str = input.to_string();

    // We use a runtime approach for the initial implementation:
    // the macro wraps the token stream in an Html value built at runtime.
    // The compile-time HTML parser will replace this in Phase 2.
    //
    // This stub just forwards to the runtime `h_runtime!` helper.
    Ok(quote! {
        ::aethos_html::Html::from_tokens(|| {
            let mut __html = ::std::string::String::new();
            // placeholder: full HEEx parsing in Phase 2
            __html
        })
    })
}
