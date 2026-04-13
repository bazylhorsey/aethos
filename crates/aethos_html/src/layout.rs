use crate::Html;

/// Wraps `inner_content` in the root HTML shell.
///
/// Analogous to Phoenix's `root.html.heex`. Applications override this by
/// calling `Endpoint::root_layout(...)` or by providing a custom `Layouts::root`
/// function component.
pub fn default_root_layout(inner_content: Html, title: Option<&str>) -> Html {
    let title = title.unwrap_or("Aethos App");
    Html::new(format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>{title}</title>
  <meta name="csrf-token" content="" />
</head>
<body>
  <main>
    {inner}
  </main>
  <script type="module" src="/_aethos/aethos.js"></script>
</body>
</html>"#,
        inner = inner_content.as_str()
    ))
}

/// An application-level layout component.
///
/// Typical usage in a function component:
/// ```rust,ignore
/// fn app_layout(assigns: Assigns) -> Html {
///     let inner = assigns.get::<InnerContent>().cloned().unwrap_or_default();
///     Html::new(format!("<div class=\"app\">{}</div>", inner.0.as_str()))
/// }
/// ```
pub struct InnerContent(pub Html);

impl Default for InnerContent {
    fn default() -> Self {
        InnerContent(Html::empty())
    }
}
