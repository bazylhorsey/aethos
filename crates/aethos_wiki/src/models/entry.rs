use std::path::{Path, PathBuf};
use std::io;
use aethos::orm::Changeset;

#[derive(Debug, Clone)]
pub struct Entry {
    pub title:   String,
    pub content: String,
}

impl Entry {
    fn file_path(dir: &Path, title: &str) -> PathBuf {
        dir.join(format!("{}.md", title))
    }

    pub async fn all(dir: &Path) -> Vec<Self> {
        let mut entries = Vec::new();
        if let Ok(mut rd) = tokio::fs::read_dir(dir).await {
            while let Ok(Some(ent)) = rd.next_entry().await {
                let path = ent.path();
                if path.extension().and_then(|e| e.to_str()) == Some("md") {
                    if let Some(title) = path.file_stem().and_then(|s| s.to_str()) {
                        if let Ok(content) = tokio::fs::read_to_string(&path).await {
                            entries.push(Entry { title: title.to_owned(), content });
                        }
                    }
                }
            }
        }
        entries.sort_unstable_by(|a, b| a.title.cmp(&b.title));
        entries
    }

    pub async fn find_by_title(dir: &Path, title: &str) -> Option<Self> {
        tokio::fs::read_to_string(Self::file_path(dir, title)).await.ok().map(|content| {
            Entry { title: title.to_owned(), content }
        })
    }

    pub async fn search(dir: &Path, query: &str) -> Vec<Self> {
        let lower = query.to_lowercase();
        Self::all(dir).await.into_iter()
            .filter(|e| e.title.to_lowercase().contains(&lower))
            .collect()
    }

    pub fn changeset(title: Option<&str>, content: Option<&str>) -> Changeset {
        Changeset::new()
            .cast_str("title", title)
            .cast_str("content", content)
            .validate_required("title")
            .validate_length("title", 1, 200)
            .validate_required("content")
    }

    pub async fn create(dir: &Path, title: &str, content: &str) -> io::Result<()> {
        let path = Self::file_path(dir, title);
        if path.exists() {
            return Err(io::Error::new(io::ErrorKind::AlreadyExists, "page already exists"));
        }
        tokio::fs::write(path, content).await
    }

    pub async fn update(dir: &Path, title: &str, content: &str) -> io::Result<()> {
        tokio::fs::write(Self::file_path(dir, title), content).await
    }
}
