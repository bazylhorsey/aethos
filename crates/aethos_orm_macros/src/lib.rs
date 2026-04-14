//! Proc-macro derive for the `aethos_orm::Schema` trait.
//!
//! Generates `impl Schema for MyStruct` with:
//! - `table_name()` — from `#[schema(table = "my_table")]`  
//! - `primary_key()` — from the field marked `#[field(primary_key)]`
//! - `columns()` — all non-primary-key, non-skipped field names
//! - `to_row_values()` — direct field-to-`SqlValue` conversion (no JSON)
//!
//! # Usage
//!
//! ```rust,ignore
//! use aethos_orm::{Schema, SqlValue};
//! use sqlx::FromRow;
//!
//! #[derive(Debug, Schema, FromRow)]
//! #[schema(table = "users")]
//! pub struct User {
//!     #[field(primary_key)]
//!     pub id:    i64,
//!     pub name:  String,
//!     pub email: String,
//! }
//! // Generates:
//! // impl Schema for User {
//! //     fn table_name() -> &'static str { "users" }
//! //     fn primary_key() -> &'static str { "id" }
//! //     fn columns() -> &'static [&'static str] { &["name", "email"] }
//! //     fn to_row_values(&self) -> Vec<SqlValue> {
//! //         vec![self.name.clone().into(), self.email.clone().into()]
//! //     }
//! // }
//! ```

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields};

#[proc_macro_derive(Schema, attributes(schema, field))]
pub fn derive_schema(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand_schema(input) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

fn expand_schema(input: DeriveInput) -> syn::Result<TokenStream2> {
    let name = &input.ident;

    // Derive the table name from #[schema(table = "...")] or snake_plural(TypeName)
    let table_name = extract_schema_attr(&input.attrs, "table")
        .unwrap_or_else(|| to_snake_plural(&name.to_string()));

    let fields = match &input.data {
        Data::Struct(s) => match &s.fields {
            Fields::Named(f) => &f.named,
            _ => return Err(syn::Error::new_spanned(name, "Schema requires named fields")),
        },
        _ => return Err(syn::Error::new_spanned(name, "Schema can only be derived for structs")),
    };

    let mut primary_key = "id".to_string();
    let mut pk_ident: Option<proc_macro2::Ident> = None;
    let mut insert_cols: Vec<String>  = Vec::new();
    let mut value_exprs: Vec<TokenStream2> = Vec::new();

    for field in fields {
        let ident = field.ident.as_ref().unwrap();
        let col   = ident.to_string();

        let is_pk   = has_field_flag(field, "primary_key");
        let is_skip = has_field_flag(field, "skip");

        if is_skip { continue; }

        if is_pk {
            primary_key = col;
            pk_ident = Some(ident.clone());
            continue;
        }

        insert_cols.push(col);
        value_exprs.push(quote! {
            ::aethos_orm::SqlValue::from(::std::clone::Clone::clone(&self.#ident))
        });
    }

    let col_literals: Vec<_> = insert_cols.iter().map(|c| quote!(#c)).collect();

    let pk_value_impl = if let Some(pk) = pk_ident {
        quote! {
            fn primary_key_value(&self) -> ::aethos_orm::SqlValue {
                ::aethos_orm::SqlValue::from(::std::clone::Clone::clone(&self.#pk))
            }
        }
    } else {
        quote! {}
    };

    Ok(quote! {
        impl ::aethos_orm::Schema for #name {
            fn table_name() -> &'static str { #table_name }
            fn primary_key() -> &'static str { #primary_key }
            fn columns() -> &'static [&'static str] {
                &[#(#col_literals),*]
            }
            fn to_row_values(&self) -> ::std::vec::Vec<::aethos_orm::SqlValue> {
                vec![#(#value_exprs),*]
            }
            #pk_value_impl
        }
    })
}

// ── Attribute helpers ─────────────────────────────────────────────────────────

/// Extract a string value from `#[schema(key = "value")]`.
fn extract_schema_attr(attrs: &[syn::Attribute], key: &str) -> Option<String> {
    for attr in attrs {
        if !attr.path().is_ident("schema") { continue; }
        let mut result = None;
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident(key) {
                let value = meta.value()?;
                let lit: syn::LitStr = value.parse()?;
                result = Some(lit.value());
            }
            Ok(())
        });
        if result.is_some() { return result; }
    }
    None
}

/// Check whether `#[field(flag_name)]` is present on a field.
fn has_field_flag(field: &syn::Field, flag: &str) -> bool {
    for attr in &field.attrs {
        if !attr.path().is_ident("field") { continue; }
        let mut found = false;
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident(flag) { found = true; }
            Ok(())
        });
        if found { return true; }
    }
    false
}

// ── Naming helpers ────────────────────────────────────────────────────────────

/// `UserProfile` → `user_profiles`
fn to_snake_plural(name: &str) -> String {
    let mut s = String::with_capacity(name.len() + 4);
    for (i, c) in name.chars().enumerate() {
        if c.is_uppercase() && i > 0 { s.push('_'); }
        s.push(c.to_ascii_lowercase());
    }
    s.push('s');
    s
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snake_plural() {
        assert_eq!(to_snake_plural("User"),        "users");
        assert_eq!(to_snake_plural("UserProfile"), "user_profiles");
        assert_eq!(to_snake_plural("Post"),        "posts");
    }
}
