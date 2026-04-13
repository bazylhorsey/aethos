use proc_macro::TokenStream;

mod router_macro;
mod html_macro;
mod controller_macro;
mod path_macro;

/// Declare the application router with pipelines, scopes, and routes.
///
/// ```rust,ignore
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
/// # Basic usage
/// ```rust,ignore
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
///
/// # Named slots
/// ```rust,ignore
/// // Component definition:
/// fn card(assigns: &Assigns) -> Html {
///     h! {
///         <div class="card">
///             <div class="card-header">{raw(assigns.slot("header"))}</div>
///             <div class="card-body">{raw(assigns.slot("inner_block"))}</div>
///         </div>
///     }
/// }
///
/// // Usage:
/// fn page(assigns: &Assigns) -> Html {
///     h! {
///         <.card>
///             <:header><h2>My Title</h2></:header>
///             <p>Card content here</p>
///         </.card>
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

/// Build a URL path with interpolated parameters.
///
/// Unlike `format!`, this uses Phoenix-style `:param` syntax, making it easy
/// to read alongside `router!` route patterns.
///
/// ```rust,ignore
/// // Simple path
/// let url = path!("/users");
/// assert_eq!(url, "/users");
///
/// // With a single param
/// let id = 42u32;
/// let url = path!("/users/:id", id = id);
/// assert_eq!(url, "/users/42");
///
/// // With multiple params
/// let url = path!("/posts/:post_id/comments/:id", post_id = 5, id = 10);
/// assert_eq!(url, "/posts/5/comments/10");
/// ```
#[proc_macro]
pub fn path(input: TokenStream) -> TokenStream {
    path_macro::expand(input.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}
