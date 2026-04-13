use aethos::Conn;
use pulldown_cmark::{Parser, Options, html as cmark_html};

use crate::models::Entry;
use crate::state::AppState;
use crate::templates::{layout, pages};

pub struct PageController;

fn get_state(conn: &Conn) -> AppState {
    conn.request
        .extensions()
        .get::<AppState>()
        .cloned()
        .expect("AppState not found in request extensions")
}

fn markdown_to_html(md: &str) -> String {
    let parser = Parser::new_ext(md, Options::all());
    let mut html_out = String::new();
    cmark_html::push_html(&mut html_out, parser);
    html_out
}

impl PageController {
    /// GET / — list all entries
    pub async fn index(conn: Conn) -> Conn {
        let state = get_state(&conn);
        let entries = Entry::all(&state.repo).await;
        let content = pages::index_page(&entries);
        conn.html(layout::render_page("Home", &content))
    }

    /// GET /wiki/:title — show entry
    pub async fn show(conn: Conn) -> Conn {
        let state = get_state(&conn);
        let title = conn.params.get("title").unwrap_or("").to_owned();
        match Entry::find_by_title(&state.repo, &title).await {
            Some(entry) => {
                let html_content = markdown_to_html(&entry.content);
                let content = pages::show_page(&entry, &html_content);
                conn.html(layout::render_page(&title, &content))
            }
            None => {
                let content = pages::not_found_page(&title);
                conn.put_status(aethos::http::StatusCode::NOT_FOUND)
                    .html(layout::render_page("Not Found", &content))
            }
        }
    }

    /// GET /wiki/new — new entry form
    pub async fn new_form(conn: Conn) -> Conn {
        let prefill_title = conn.params.get("title").unwrap_or("").to_owned();
        let content = pages::new_form(None, &prefill_title, "");
        conn.html(layout::render_page("New Page", &content))
    }

    /// POST /wiki/new — create entry
    pub async fn create(conn: Conn) -> Conn {
        let state = get_state(&conn);
        let title = conn.params.get("title").unwrap_or("").trim().to_owned();
        let body = conn.params.get("content").unwrap_or("").to_owned();

        if title.is_empty() {
            let content = pages::new_form(Some("Title cannot be empty."), &title, &body);
            return conn
                .put_status(aethos::http::StatusCode::UNPROCESSABLE_ENTITY)
                .html(layout::render_page("New Page", &content));
        }

        if Entry::find_by_title(&state.repo, &title).await.is_some() {
            let msg = format!("An entry titled \"{}\" already exists.", title);
            let content = pages::new_form(Some(&msg), &title, &body);
            return conn
                .put_status(aethos::http::StatusCode::UNPROCESSABLE_ENTITY)
                .html(layout::render_page("New Page", &content));
        }

        match Entry::create(&state.repo, &title, &body).await {
            Ok(()) => {
                let redirect_url = format!("/wiki/{}", layout::url_encode(&title));
                conn.redirect(redirect_url)
            }
            Err(e) => {
                let msg = format!("Failed to create entry: {e}");
                let content = pages::new_form(Some(&msg), &title, &body);
                conn.put_status(aethos::http::StatusCode::INTERNAL_SERVER_ERROR)
                    .html(layout::render_page("New Page", &content))
            }
        }
    }

    /// GET /wiki/:title/edit — edit form
    pub async fn edit_form(conn: Conn) -> Conn {
        let state = get_state(&conn);
        let title = conn.params.get("title").unwrap_or("").to_owned();
        match Entry::find_by_title(&state.repo, &title).await {
            Some(entry) => {
                let page_title = format!("Edit: {}", title);
                let content = pages::edit_form(&entry, None);
                conn.html(layout::render_page(&page_title, &content))
            }
            None => {
                let content = pages::not_found_page(&title);
                conn.put_status(aethos::http::StatusCode::NOT_FOUND)
                    .html(layout::render_page("Not Found", &content))
            }
        }
    }

    /// POST /wiki/:title/edit — update entry
    pub async fn update(conn: Conn) -> Conn {
        let state = get_state(&conn);
        let title = conn.params.get("title").unwrap_or("").to_owned();
        let body = conn.params.get("content").unwrap_or("").to_owned();

        match Entry::find_by_title(&state.repo, &title).await {
            Some(entry) => match Entry::update(&state.repo, &title, &body).await {
                Ok(()) => {
                    let redirect_url = format!("/wiki/{}", layout::url_encode(&title));
                    conn.redirect(redirect_url)
                }
                Err(e) => {
                    let msg = format!("Failed to save: {e}");
                    let page_title = format!("Edit: {}", title);
                    let content = pages::edit_form(&entry, Some(&msg));
                    conn.put_status(aethos::http::StatusCode::INTERNAL_SERVER_ERROR)
                        .html(layout::render_page(&page_title, &content))
                }
            },
            None => {
                let content = pages::not_found_page(&title);
                conn.put_status(aethos::http::StatusCode::NOT_FOUND)
                    .html(layout::render_page("Not Found", &content))
            }
        }
    }

    /// GET /search?q=... — search entries
    pub async fn search(conn: Conn) -> Conn {
        let state = get_state(&conn);
        let query = conn.params.get("q").unwrap_or("").trim().to_owned();

        if query.is_empty() {
            return conn.redirect("/");
        }

        let results = Entry::search(&state.repo, &query).await;

        // Exact case-insensitive match → redirect directly
        let exact = results
            .iter()
            .find(|e| e.title.to_lowercase() == query.to_lowercase());

        if let Some(entry) = exact {
            let redirect_url = format!("/wiki/{}", layout::url_encode(&entry.title));
            return conn.redirect(redirect_url);
        }

        let content = pages::search_results_page(&query, &results);
        conn.html(layout::render_page("Search Results", &content))
    }

    /// GET /random — redirect to a random entry
    pub async fn random(conn: Conn) -> Conn {
        let state = get_state(&conn);
        let entries = Entry::all(&state.repo).await;
        if entries.is_empty() {
            return conn.redirect("/");
        }
        let idx = (rand::random::<u64>() as usize) % entries.len();
        let redirect_url = format!("/wiki/{}", layout::url_encode(&entries[idx].title));
        conn.redirect(redirect_url)
    }
}
