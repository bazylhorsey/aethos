pub mod html;
pub mod assigns;
pub mod escape;

pub use html::Html;
pub use assigns::Assigns;
pub use escape::html_escape;

pub use aethos_macros::h;
