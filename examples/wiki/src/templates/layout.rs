pub fn render_page(title: &str, content: &str) -> String {
    format!(r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>{title} - Aethos Wiki</title>
  <link rel="stylesheet" href="/assets/wiki.css" />
</head>
<body>
  <header class="wiki-header">
    <a href="/" class="wiki-logo">Aethos Wiki</a>
  </header>
  <div class="wiki-body">
    <nav class="wiki-sidebar">
      <form action="/search" method="get" class="search-form">
        <input type="text" name="q" placeholder="Search wiki..." class="search-input" />
        <button type="submit" class="btn btn-search">Search</button>
      </form>
      <ul class="sidebar-nav">
        <li><a href="/">Home</a></li>
        <li><a href="/wiki/new">New Page</a></li>
        <li><a href="/random">Random Page</a></li>
      </ul>
    </nav>
    <main class="wiki-main">
      {content}
    </main>
  </div>
</body>
</html>"#)
}
