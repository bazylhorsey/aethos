use proc_macro::TokenStream;

mod router_macro;
mod html_macro;
mod controller_macro;

/// Declare the application router with pipelines, scopes, and routes.
///
/// ```rust
/// router! {
///     pipeline :browser {
///         plug!(Logger);
///         plug!(SecureHeaders);
///     }
///     scope "/" {
///         pipe_through!(:browser);
///         get!("/", PageController, index);
///         get!("/hello/:name", HelloController, show);
///         resources!("/users", UserController);
///         live!("/thermostat", ThermostatLive);
///     }
/// }
/// ```
#[proc_macro]
pub fn router(input: TokenStream) -> TokenStream {
    router_macro::expand(input.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

/// HEEx-inspired HTML template macro.
///
/// ```rust
/// fn my_view(assigns: &Assigns) -> Html {
///     h! {
///         <div class={@class}>
///             <h1>{@title}</h1>
///             <p :if={@show_note}>{@note}</p>
///             <ul>
///                 <li :for={item in @items}>{item.name}</li>
///             </ul>
///             <.flash_messages flash={@flash} />
///         </div>
///     }
/// }
/// ```
#[proc_macro]
pub fn h(input: TokenStream) -> TokenStream {
    html_macro::expand(input.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

/// Derive a Phoenix-style controller struct.
#[proc_macro_attribute]
pub fn controller(_attr: TokenStream, item: TokenStream) -> TokenStream {
    controller_macro::expand(item.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}
