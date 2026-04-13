use aethos_core::{Conn, plugs::csrf::CsrfToken};
use crate::Html;

/// Newtype for a page title stored in `conn.assigns`.
pub struct PageTitle(pub String);

/// Extension trait that adds `conn.render(html)` as a convenience over `conn.html(...)`.
pub trait ConnHtmlExt {
    /// Wrap `html` in the default root layout and send as an HTML response.
    ///
    /// Reads `CsrfToken` and `PageTitle` from `conn.assigns` if present.
    fn render(self, html: Html) -> Conn;
}

impl ConnHtmlExt for Conn {
    fn render(self, html: Html) -> Conn {
        let title = self.assigns.get::<PageTitle>().map(|t| t.0.clone());
        let csrf = self.assigns.get::<CsrfToken>().map(|t| t.0.clone());

        let full = crate::default_root_layout(
            html,
            title.as_deref(),
            csrf.as_deref(),
        );
        self.html(full.0)
    }
}
