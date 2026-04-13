pub mod html;
pub mod assigns;
pub mod escape;
pub mod layout;
pub mod conn_ext;
pub mod template;

pub use html::{Html, Safe};
pub use assigns::Assigns;
pub use escape::html_escape;
pub use layout::{default_root_layout, InnerContent};
pub use conn_ext::ConnHtmlExt;
pub use template::Template;

pub use aethos_macros::h;
