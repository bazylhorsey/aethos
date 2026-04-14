pub fn render_page(title: &str, content: &str) -> String {
    let mut out = String::with_capacity(4096);
    out.push_str("<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n");
    out.push_str("  <meta charset=\"utf-8\" />\n");
    out.push_str("  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\" />\n");
    out.push_str("  <title>");
    out.push_str(title);
    out.push_str(" - Aethos Wiki</title>\n");
    out.push_str("  <link rel=\"stylesheet\" href=\"/assets/wiki.css\" />\n");
    out.push_str("</head>\n<body>\n");

    // header
    out.push_str(
        "  <header class=\"wiki-header\">\
         <a href=\"/\" class=\"wiki-logo\">Aethos Wiki</a>\
         </header>\n",
    );

    // body wrapper
    out.push_str("  <div class=\"wiki-body\">\n");

    // sidebar
    out.push_str(
        "    <nav class=\"wiki-sidebar\">\
         <form action=\"/search\" method=\"get\" class=\"search-form\">\
         <input type=\"text\" name=\"q\" placeholder=\"Search wiki...\" class=\"search-input\" />\
         <button type=\"submit\" class=\"btn btn-search\">Search</button>\
         </form>\
         <ul class=\"sidebar-nav\">\
         <li><a href=\"/\">Home</a></li>\
         <li><a href=\"/wiki/new\">New Page</a></li>\
         <li><a href=\"/random\">Random Page</a></li>\
         </ul>\
         </nav>\n",
    );

    // main content
    out.push_str("    <main class=\"wiki-main\">\n");
    out.push_str(content);
    out.push_str("\n    </main>\n");

    out.push_str("  </div>\n</body>\n</html>");
    out
}
