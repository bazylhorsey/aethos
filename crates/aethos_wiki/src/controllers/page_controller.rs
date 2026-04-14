use aethos::{Conn, path, url_encode};
use aethos::http::StatusCode;
use aethos::orm::Changeset;
use pulldown_cmark::{Parser, Options, html as cmark_html};

use crate::models::Entry;
use crate::state::AppState;
use crate::templates::{layout, pages};

pub struct PageController;

fn state(conn: &Conn) -> AppState {
    conn.get_assign::<AppState>()
        .cloned()
        .expect("AppState not in assigns — is FetchState plug installed?")
}

fn markdown_to_html(md: &str) -> String {
    let parser = Parser::new_ext(md, Options::all());
    let mut out = String::new();
    cmark_html::push_html(&mut out, parser);
    out
}

impl PageController {
    /// GET /
    pub async fn index(conn: Conn) -> Conn {
        let entries = Entry::all(&state(&conn).content_dir).await;
        conn.html(layout::render_page("Home", &pages::index_page(&entries)))
    }

    /// GET /wiki/:title
    pub async fn show(conn: Conn) -> Conn {
        let title = conn.params.get("title").unwrap_or("").to_owned();
        let dir = state(&conn).content_dir;
        match Entry::find_by_title(&dir, &title).await {
            Some(entry) => {
                let body = pages::show_page(&entry, &markdown_to_html(&entry.content));
                conn.html(layout::render_page(&title, &body))
            }
            None => conn.put_status(StatusCode::NOT_FOUND)
                .html(layout::render_page("Not Found", &pages::not_found_page(&title))),
        }
    }

    /// GET /wiki/new
    pub async fn new_form(conn: Conn) -> Conn {
        conn.html(layout::render_page("New Page", &pages::new_form(None)))
    }

    /// POST /wiki/new
    pub async fn create(conn: Conn) -> Conn {
        let dir = state(&conn).content_dir;
        let cs  = Entry::changeset(conn.params.get("title"), conn.params.get("content"));

        if !cs.is_valid() {
            return conn.put_status(StatusCode::UNPROCESSABLE_ENTITY)
                .html(layout::render_page("New Page", &pages::new_form(Some(&cs))));
        }

        let data  = cs.apply().unwrap();
        let title = data["title"].clone();
        let body  = data["content"].clone();

        match Entry::create(&dir, &title, &body).await {
            Ok(()) => conn.redirect(path!("/wiki/:title", title = url_encode(&title))),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                let dup_cs = Changeset::new()
                    .cast_str("title", Some(title.as_str()))
                    .cast_str("content", Some(body.as_str()))
                    .validate_with("title", |_| {
                        Some(format!("A page titled \"{}\" already exists.", title))
                    });
                conn.put_status(StatusCode::UNPROCESSABLE_ENTITY)
                    .html(layout::render_page("New Page", &pages::new_form(Some(&dup_cs))))
            }
            Err(e) => {
                let err_cs = Changeset::new()
                    .cast_str("title", Some(title.as_str()))
                    .cast_str("content", Some(body.as_str()))
                    .validate_with("title", |_| Some(format!("Failed to save: {e}")));
                conn.put_status(StatusCode::INTERNAL_SERVER_ERROR)
                    .html(layout::render_page("New Page", &pages::new_form(Some(&err_cs))))
            }
        }
    }

    /// GET /wiki/:title/edit
    pub async fn edit_form(conn: Conn) -> Conn {
        let title = conn.params.get("title").unwrap_or("").to_owned();
        let dir   = state(&conn).content_dir;
        match Entry::find_by_title(&dir, &title).await {
            Some(entry) => conn.html(layout::render_page(
                &format!("Edit: {title}"),
                &pages::edit_form(&entry, None),
            )),
            None => conn.put_status(StatusCode::NOT_FOUND)
                .html(layout::render_page("Not Found", &pages::not_found_page(&title))),
        }
    }

    /// POST /wiki/:title/edit
    pub async fn update(conn: Conn) -> Conn {
        let title = conn.params.get("title").unwrap_or("").to_owned();
        let dir   = state(&conn).content_dir;
        let cs    = Entry::changeset(Some(&title), conn.params.get("content"));

        match Entry::find_by_title(&dir, &title).await {
            None => conn.put_status(StatusCode::NOT_FOUND)
                .html(layout::render_page("Not Found", &pages::not_found_page(&title))),
            Some(entry) => {
                if !cs.is_valid() {
                    return conn.put_status(StatusCode::UNPROCESSABLE_ENTITY)
                        .html(layout::render_page(&format!("Edit: {title}"), &pages::edit_form(&entry, Some(&cs))));
                }
                let data = cs.apply().unwrap();
                match Entry::update(&dir, &title, &data["content"]).await {
                    Ok(()) => conn.redirect(path!("/wiki/:title", title = url_encode(&title))),
                    Err(e) => {
                        let err_cs = Changeset::new()
                            .cast_str("title", Some(title.as_str()))
                            .cast_str("content", Some(data["content"].as_str()))
                            .validate_with("content", |_| Some(format!("Failed to save: {e}")));
                        conn.put_status(StatusCode::INTERNAL_SERVER_ERROR)
                            .html(layout::render_page(&format!("Edit: {title}"), &pages::edit_form(&entry, Some(&err_cs))))
                    }
                }
            }
        }
    }

    /// GET /search?q=...
    pub async fn search(conn: Conn) -> Conn {
        let query = conn.params.get("q").unwrap_or("").trim().to_owned();
        if query.is_empty() { return conn.redirect("/"); }

        let dir     = state(&conn).content_dir;
        let results = Entry::search(&dir, &query).await;
        if let Some(exact) = results.iter().find(|e| e.title.eq_ignore_ascii_case(&query)) {
            return conn.redirect(path!("/wiki/:title", title = url_encode(&exact.title)));
        }
        conn.html(layout::render_page("Search Results", &pages::search_results_page(&query, &results)))
    }

    /// GET /random
    pub async fn random(conn: Conn) -> Conn {
        let entries = Entry::all(&state(&conn).content_dir).await;
        if entries.is_empty() { return conn.redirect("/"); }
        let idx = (rand::random::<u64>() as usize) % entries.len();
        conn.redirect(path!("/wiki/:title", title = url_encode(&entries[idx].title)))
    }
}

