use std::path::PathBuf;

/// Application state — path to the markdown content directory.
#[derive(Clone)]
pub struct AppState {
    pub content_dir: PathBuf,
}
