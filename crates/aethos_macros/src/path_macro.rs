/// `path!` — Phoenix-style URL path builder.
///
/// Transforms `:param` segments into `{param}` format! placeholders and
/// generates a `format!` call with the provided bindings.
///
/// `path!("/users/:id", id = user.id)` → `format!("/users/{id}", id = user.id)`
use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    LitStr, Token, Ident, Expr, Result,
};

struct PathInput {
    pattern: LitStr,
    args: Vec<(Ident, Expr)>,
}

impl Parse for PathInput {
    fn parse(input: ParseStream) -> Result<Self> {
        let pattern: LitStr = input.parse()?;
        let mut args = Vec::new();
        while input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
            if input.is_empty() { break; }
            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            let val: Expr = input.parse()?;
            args.push((key, val));
        }
        Ok(PathInput { pattern, args })
    }
}

pub fn expand(input: TokenStream) -> Result<TokenStream> {
    let PathInput { pattern, args } = syn::parse2(input)?;

    let raw = pattern.value();

    // Replace `:name` segments with `{name}` for format!
    let fmt_str = replace_params(&raw);

    if args.is_empty() {
        // No bindings — emit a string literal if possible (no params to fill)
        if fmt_str == raw {
            return Ok(quote! { #fmt_str });
        }
        // Has placeholders but no bindings provided — emit the format string
        // (caller will get a compile error from format! if params are missing)
        return Ok(quote! { ::std::format!(#fmt_str) });
    }

    let keys: Vec<&Ident> = args.iter().map(|(k, _)| k).collect();
    let vals: Vec<&Expr>  = args.iter().map(|(_, v)| v).collect();

    Ok(quote! {
        ::std::format!(#fmt_str, #( #keys = #vals ),*)
    })
}

/// Replace `:name` path segments with `{name}` format! placeholders.
fn replace_params(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    let mut chars = path.chars().peekable();
    while let Some(c) = chars.next() {
        if c == ':' {
            // Collect the parameter name (alphanumeric + underscore)
            let mut name = String::new();
            while let Some(&nc) = chars.peek() {
                if nc.is_alphanumeric() || nc == '_' {
                    name.push(nc);
                    chars.next();
                } else {
                    break;
                }
            }
            if name.is_empty() {
                out.push(':'); // bare colon — keep as-is
            } else {
                out.push('{');
                out.push_str(&name);
                out.push('}');
            }
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::replace_params;

    #[test]
    fn test_replace_params() {
        assert_eq!(replace_params("/users"), "/users");
        assert_eq!(replace_params("/users/:id"), "/users/{id}");
        assert_eq!(replace_params("/posts/:post_id/comments/:id"), "/posts/{post_id}/comments/{id}");
        assert_eq!(replace_params("/:a/:b/:c"), "/{a}/{b}/{c}");
    }
}
