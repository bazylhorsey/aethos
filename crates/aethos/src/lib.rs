/// Aethos — Phoenix-inspired web framework for Rust
///
/// Re-exports all public types from the framework crates for ergonomic one-line imports.
pub use aethos_core::{
    Conn, Plug, Next, BoxPlug, Params, FlashMap, ResponseBody, AethosError, TypeMap,
    async_trait, axum, bytes, http, serde, serde_json, tower, tracing,
    Telemetry, url_encode,
    Supervisor, SupervisorStrategy, SupervisorHandle, ChildSpec, RestartConfig, RestartPolicy, DynamicSupervisor,
};

pub use aethos_router::{Pipeline, Endpoint};

pub use aethos_html::{Html, Assigns, html_escape, Safe, default_root_layout, InnerContent, ConnHtmlExt, Template};
pub use aethos_html::conn_ext::PageTitle;

pub use aethos_pubsub::{PubSub, Message};

pub use aethos_channels::{Socket, Channel, handle_socket};
/// Re-export channel sub-types
pub mod channel {
    pub use aethos_channels::channel::{JoinResult, JoinError};
}

pub use aethos_presence::Presence;

pub use aethos_live::{LiveView, LiveSocket, handle_live_socket};

// Built-in plugs
pub use aethos_core::plugs::{
    Logger, RequestId, SecureHeaders, BodyParser,
    FetchSession, Csrf, CsrfToken,
    MethodOverride, AcceptsMime, StaticFiles,
};
pub use aethos_core::{Session, set_session_secret};

// Proc macros
pub use aethos_macros::{router, h, controller, path};

// Re-export assigns! and h! convenience macros
pub use aethos_html::assigns;

/// Database access — schema, repo, changeset, query DSL, migrations.
pub mod orm {
    pub use aethos_orm::{Repo, PoolConfig, Schema, Changeset, ChangesetError, Query, MigrationRunner, OrmError, SqlValue, sqlx, chrono, uuid};
}

/// Renders per-field validation errors as an HTML `<span>` element, analogous to
/// Phoenix's `error_tag/2`. Returns an empty string when there are no errors.
///
/// # Example
/// ```rust,ignore
/// h! {
///     <div class="form-group">
///         <input type="text" name="title" value={cs.get("title").unwrap_or("")} />
///         {raw(field_errors(&cs, "title"))}
///     </div>
/// }
/// ```
pub fn field_errors(cs: &aethos_orm::Changeset, field: &str) -> String {
    let errors = cs.errors_for(field);
    if errors.is_empty() {
        return String::new();
    }
    let mut html = String::from(r#"<span class="field-error">"#);
    let msgs: Vec<String> = errors.iter()
        .map(|e| aethos_html::html_escape(&e.message))
        .collect();
    html.push_str(&msgs.join("; "));
    html.push_str("</span>");
    html
}

/// Telemetry event types and helpers.
pub mod telemetry {
    pub use aethos_core::telemetry::{Event, Telemetry, elapsed_ms, system_time_ms};
}
