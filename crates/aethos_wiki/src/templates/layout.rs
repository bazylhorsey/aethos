/// Encode a string for use in a URL path segment (percent-encode special chars).
pub fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9'
            | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            b' ' => out.push_str("%20"),
            other => {
                out.push('%');
                out.push(char::from_digit((other >> 4) as u32, 16).unwrap().to_ascii_uppercase());
                out.push(char::from_digit((other & 0xf) as u32, 16).unwrap().to_ascii_uppercase());
            }
        }
    }
    out
}

const WIKI_CSS: &str = r#"
*,*::before,*::after{box-sizing:border-box;margin:0;padding:0}
body{font-family:Georgia,serif;font-size:16px;color:#202122;background:#f8f9fa}
a{color:#3366cc;text-decoration:none}
a:hover{text-decoration:underline}
.wiki-header{background:#fff;border-bottom:1px solid #a7d7f9;padding:8px 16px;display:flex;align-items:center}
.wiki-logo{font-size:1.4em;font-weight:bold;color:#202122}
.wiki-body{display:flex;min-height:calc(100vh - 50px)}
.wiki-sidebar{width:220px;background:#f8f9fa;border-right:1px solid #eaecf0;padding:12px;flex-shrink:0}
.wiki-main{flex:1;padding:24px 32px;max-width:900px}
.search-form{margin-bottom:16px}
.search-input{width:100%;padding:6px 8px;border:1px solid #a2a9b1;border-radius:2px;font-size:.9em}
.btn{display:inline-block;padding:6px 12px;border:1px solid;border-radius:2px;cursor:pointer;font-size:.9em;text-decoration:none}
.btn-search{background:#f8f9fa;border-color:#a2a9b1;margin-top:4px;width:100%}
.btn-primary{background:#3366cc;color:#fff;border-color:#2a4b8d}
.btn-secondary{background:#f8f9fa;color:#202122;border-color:#a2a9b1}
.sidebar-nav{list-style:none;margin-top:8px}
.sidebar-nav li{margin-bottom:4px}
.entry-title{font-size:2em;font-weight:normal;border-bottom:1px solid #a2a9b1;margin-bottom:16px;padding-bottom:4px}
.entry-actions{margin-bottom:16px}
.entry-content{line-height:1.6}
.entry-content h1,.entry-content h2,.entry-content h3{margin-top:16px;margin-bottom:8px;border-bottom:1px solid #eaecf0;padding-bottom:4px}
.entry-content p{margin-bottom:12px}
.entry-content ul,.entry-content ol{margin-left:24px;margin-bottom:12px}
.entry-content code{background:#f0f0f0;padding:2px 4px;border-radius:2px;font-family:monospace}
.entry-content pre{background:#f0f0f0;padding:12px;border-radius:2px;overflow-x:auto;margin-bottom:12px}
.entry-list{list-style:none}
.entry-list li{margin-bottom:6px;padding:6px 0;border-bottom:1px solid #eaecf0}
.form-group{margin-bottom:16px}
.form-label{display:block;margin-bottom:4px;font-weight:bold}
.form-control{width:100%;padding:8px;border:1px solid #a2a9b1;border-radius:2px;font-size:1em;font-family:inherit}
textarea.form-control{font-family:monospace;min-height:300px;resize:vertical}
.alert{padding:12px 16px;border-radius:2px;margin-bottom:16px}
.alert-error{background:#fee;border:1px solid #c33;color:#600}
.not-found{text-align:center;padding:48px}
.not-found h1{font-size:3em;margin-bottom:16px;color:#a2a9b1}
.search-result-item{padding:8px 0;border-bottom:1px solid #eaecf0}
.no-results{color:#72777d;font-style:italic;margin-top:16px}
"#;

pub fn render_page(title: &str, content: &str) -> String {
    let mut out = String::with_capacity(8192);
    out.push_str("<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n");
    out.push_str("  <meta charset=\"utf-8\" />\n");
    out.push_str("  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\" />\n");
    out.push_str("  <title>");
    out.push_str(title);
    out.push_str(" - Aethos Wiki</title>\n  <style>");
    out.push_str(WIKI_CSS);
    out.push_str("  </style>\n</head>\n<body>\n");

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
