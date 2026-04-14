use aethos::{h, html_escape, url_encode, field_errors};
use aethos::orm::Changeset;
use crate::models::Entry;

pub fn index_page(entries: &[Entry]) -> String {
    let list = if entries.is_empty() {
        "<p class=\"no-results\">No entries yet. Create the first one!</p>".to_string()
    } else {
        let items: String = entries
            .iter()
            .map(|e| format!(
                "<li class=\"entry-list-item\"><a href=\"/wiki/{}\">{}</a></li>",
                url_encode(&e.title),
                html_escape(&e.title),
            ))
            .collect::<Vec<_>>()
            .join("\n");
        format!("<ul class=\"entry-list\">\n{}\n</ul>", items)
    };
    h! { <div><h1 class="entry-title">All Pages</h1>{raw(list)}</div> }.render_string()
}

pub fn show_page(entry: &Entry, html_content: &str) -> String {
    let title = entry.title.as_str();
    let edit_url = format!("/wiki/{}/edit", url_encode(title));
    h! {
        <div>
            <h1 class="entry-title">{title}</h1>
            <div class="entry-actions">
                <a href={edit_url} class="btn btn-secondary">Edit</a>
            </div>
            <div class="entry-content">{raw(html_content)}</div>
        </div>
    }.render_string()
}

/// New-entry form. Pass `Some(&cs)` after a failed submission to show per-field
/// errors and repopulate field values from the changeset.
pub fn new_form(cs: Option<&Changeset>) -> String {
    let title_val   = cs.and_then(|c| c.get("title")).unwrap_or("");
    let content_val = cs.and_then(|c| c.get("content")).unwrap_or("");
    let title_errors   = cs.map(|c| field_errors(c, "title")).unwrap_or_default();
    let content_errors = cs.map(|c| field_errors(c, "content")).unwrap_or_default();
    h! {
        <div>
            <h1 class="entry-title">New Page</h1>
            <form method="post" action="/wiki/new">
                <div class="form-group">
                    <label class="form-label" for="title">Title</label>
                    <input type="text" id="title" name="title" class="form-control" value={title_val} />
                    {raw(title_errors)}
                </div>
                <div class="form-group">
                    <label class="form-label" for="content">Content (Markdown)</label>
                    <textarea id="content" name="content" class="form-control">{content_val}</textarea>
                    {raw(content_errors)}
                </div>
                <button type="submit" class="btn btn-primary">Create Page</button>
                <a href="/" class="btn btn-secondary">Cancel</a>
            </form>
        </div>
    }.render_string()
}

/// Edit form. Pass `Some(&cs)` after a failed submission for per-field errors.
pub fn edit_form(entry: &Entry, cs: Option<&Changeset>) -> String {
    let title        = entry.title.as_str();
    let content_val  = cs.and_then(|c| c.get("content")).unwrap_or(&entry.content);
    let content_errors = cs.map(|c| field_errors(c, "content")).unwrap_or_default();
    let form_action  = format!("/wiki/{}/edit", url_encode(title));
    let cancel_url   = format!("/wiki/{}", url_encode(title));
    let heading      = format!("Edit: {}", html_escape(title));
    h! {
        <div>
            <h1 class="entry-title">{raw(heading)}</h1>
            <form method="post" action={form_action}>
                <div class="form-group">
                    <label class="form-label" for="content">Content (Markdown)</label>
                    <textarea id="content" name="content" class="form-control">{content_val}</textarea>
                    {raw(content_errors)}
                </div>
                <button type="submit" class="btn btn-primary">Save Changes</button>
                <a href={cancel_url} class="btn btn-secondary">Cancel</a>
            </form>
        </div>
    }.render_string()
}

pub fn search_results_page(query: &str, results: &[Entry]) -> String {
    let results_html = if results.is_empty() {
        "<p class=\"no-results\">No entries matched your search.</p>".to_string()
    } else {
        results.iter()
            .map(|e| format!(
                "<div class=\"search-result-item\"><a href=\"/wiki/{}\">{}</a></div>",
                url_encode(&e.title),
                html_escape(&e.title),
            ))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let heading = format!("Search results for: {}", html_escape(query));
    h! {
        <div>
            <h1 class="entry-title">{raw(heading)}</h1>
            <div class="search-results">{raw(results_html)}</div>
        </div>
    }.render_string()
}

pub fn not_found_page(title: &str) -> String {
    let msg        = format!("The page \"{}\" does not exist.", html_escape(title));
    let create_url = format!("/wiki/new?title={}", url_encode(title));
    h! {
        <div class="not-found">
            <h1>404</h1>
            <p>{raw(msg)}</p>
            <a href="/" class="btn btn-primary">Go Home</a>
            <a href={create_url} class="btn btn-secondary">Create It</a>
        </div>
    }.render_string()
}
