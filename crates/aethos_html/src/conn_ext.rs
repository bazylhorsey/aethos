use aethos_core::Conn;
use crate::Html;

/// Extension trait that adds `conn.render(html)` as a convenience over `conn.html(...)`.
pub trait ConnHtmlExt {
    /// Send an `Html` fragment as the response body.
    fn render(self, html: Html) -> Conn;
}

impl ConnHtmlExt for Conn {
    fn render(self, html: Html) -> Conn {
        self.html(html.0)
    }
}
