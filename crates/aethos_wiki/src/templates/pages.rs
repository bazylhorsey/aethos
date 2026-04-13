use aethos::h;
use crate::models::Entry;
use super::layout::url_encode;

pub fn index_page(entries: &[Entry]) -> String {
    let list = if entries.is_empty() {
        "<p class=\"no-results\">No entries yet. Create the first one!</p>".to_string()
    } else {
        let items: String = entries
            .iter()
            .map(|e| {
                format!(
                    "<li class=\"entry-list-item\"><a href=\"/wiki/{}\">{}</a></li>",
                    url_encode(&e.title),
                    html_escape(&e.title),
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        format!("<ul class=\"entry-list\">\n{}\n</ul>", items)
    };

    let tpl = h! {
        <div>
            <h1 class="entry-title">All Pages</h1>
            {raw(list)}
        </div>
    };
    tpl.render_string()
}

pub fn show_page(entry: &Entry, html_content: &str) -> String {
    let title = entry.title.as_str();
    let edit_url = format!("/wiki/{}/edit", url_encode(title));
    let tpl = h! {
        <div>
            <h1 class="entry-title">{title}</h1>
            <div class="entry-actions">
                <a href={edit_url} class="btn btn-secondary">Edit</a>
            </div>
            <div class="entry-content">
                {raw(html_content)}
            </div>
        </div>
    };
    tpl.render_string()
}

pub fn new_form(error: Option<&str>, prefill_title: &str, prefill_content: &str) -> String {
    let error_html = if let Some(msg) = error {
        format!("<div class=\"alert alert-error\">{}</div>", html_escape(msg))
    } else {
        String::new()
    };
    let tpl = h! {
        <div>
            <h1 class="entry-title">New Page</h1>
            {raw(error_html)}
            <form method="post" action="/wiki/new">
                <div class="form-group">
                    <label class="form-label" for="title">Title</label>
                    <input type="text" id="title" name="title" class="form-control" value={prefill_title} />
                </div>
                <div class="form-group">
                    <label class="form-label" for="content">Content (Markdown)</label>
                    <textarea id="content" name="content" class="form-control">{prefill_content}</textarea>
                </div>
                <button type="submit" class="btn btn-primary">Create Page</button>
                <a href="/" class="btn btn-secondary">Cancel</a>
            </form>
        </div>
    };
    tpl.render_string()
}

pub fn edit_form(entry: &Entry, error: Option<&str>) -> String {
    let title = entry.title.as_str();
    let content = entry.content.as_str();
    let form_action = format!("/wiki/{}/edit", url_encode(title));
    let error_html = if let Some(msg) = error {
        format!("<div class=\"alert alert-error\">{}</div>", html_escape(msg))
    } else {
        String::new()
    };
    let heading = format!("Edit: {}", html_escape(title));
    let tpl = h! {
        <div>
            <h1 class="entry-title">{raw(heading)}</h1>
            {raw(error_html)}
            <form method="post" action={form_action}>
                <div class="form-group">
                    <label class="form-label" for="content">Content (Markdown)</label>
                    <textarea id="content" name="content" class="form-control">{content}</textarea>
                </div>
                <button type="submit" class="btn btn-primary">Save Changes</button>
                <a href={format!("/wiki/{}", url_encode(title))} class="btn btn-secondary">Cancel</a>
            </form>
        </div>
    };
    tpl.render_string()
}

pub fn search_results_page(query: &str, results: &[Entry]) -> String {
    let results_html = if results.is_empty() {
        "<p class=\"no-results\">No entries matched your search.</p>".to_string()
    } else {
        let items: String = results
            .iter()
            .map(|e| {
                format!(
                    "<div class=\"search-result-item\"><a href=\"/wiki/{}\">{}</a></div>",
                    url_encode(&e.title),
                    html_escape(&e.title),
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        items
    };

    let heading = format!("Search results for: {}", html_escape(query));
    let tpl = h! {
        <div>
            <h1 class="entry-title">{raw(heading)}</h1>
            <div class="search-results">
                {raw(results_html)}
            </div>
        </div>
    };
    tpl.render_string()
}

pub fn not_found_page(title: &str) -> String {
    let msg = format!("The page {} does not exist.", html_escape(title));
    let create_url = format!("/wiki/new?title={}", url_encode(title));
    let tpl = h! {
        <div class="not-found">
            <h1>404</h1>
            <p>{raw(msg)}</p>
            <a href="/" class="btn btn-primary">Go Home</a>
            <a href={create_url} class="btn btn-secondary">Create It</a>
        </div>
    };
    tpl.render_string()
}

fn html_escape(s: &str) -> String {
    aethos::html_escape(s)
}
