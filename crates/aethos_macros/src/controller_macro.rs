/// Stub for controller attribute macro. Passes through the item unchanged for now;
/// code generation will be expanded in a later phase.
use proc_macro2::TokenStream;
use syn::Result;

pub fn expand(item: TokenStream) -> Result<TokenStream> {
    Ok(item)
}
