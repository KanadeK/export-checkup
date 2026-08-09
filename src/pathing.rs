use std::path::Path;

use crate::model::FileKind;

const MEDIA_EXTENSIONS: &[&str] = &[
    "3gp", "avi", "gif", "heic", "heif", "jpeg", "jpg", "m4v", "mkv", "mov", "mp4", "mpeg", "mpg",
    "png", "svg", "tif", "tiff", "webm", "webp",
];

#[must_use]
pub fn normalize_logical_path(input: &str) -> Option<String> {
    let replaced = input.replace('\\', "/");
    let mut parts = Vec::new();
    for part in replaced.split('/') {
        match part {
            "" | "." => {}
            ".." => return None,
            value => parts.push(value),
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

#[must_use]
pub fn canonical_export_path(input: &str) -> String {
    let normalized = normalize_logical_path(input).unwrap_or_else(|| input.replace('\\', "/"));
    let lower = normalized.to_lowercase();
    for anchor in [
        "takeout/google photos/",
        "google photos/",
        "your_instagram_activity/",
        "connections/",
        "messages/",
    ] {
        if let Some(index) = lower.find(anchor) {
            return normalized[index..].to_owned();
        }
    }
    normalized
}

#[must_use]
pub fn file_kind(path: &str) -> FileKind {
    match extension(path).as_deref() {
        Some("json") => FileKind::Json,
        Some("html" | "htm") => FileKind::Html,
        Some(extension) if MEDIA_EXTENSIONS.contains(&extension) => FileKind::Media,
        _ => FileKind::Other,
    }
}

#[must_use]
pub fn is_media(path: &str) -> bool {
    file_kind(path) == FileKind::Media
}

#[must_use]
pub fn extension(path: &str) -> Option<String> {
    Path::new(path)
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_lowercase)
}

#[must_use]
pub fn parent(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(parent, _)| parent)
}

#[must_use]
pub fn basename(path: &str) -> &str {
    path.rsplit_once('/').map_or(path, |(_, name)| name)
}

#[must_use]
pub fn join(parent: &str, child: &str) -> String {
    if parent.is_empty() {
        child.to_owned()
    } else {
        format!("{parent}/{child}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalization_rejects_parent_traversal() {
        assert_eq!(normalize_logical_path("a/../b"), None);
        assert_eq!(normalize_logical_path("./a\\b"), Some("a/b".to_owned()));
    }

    #[test]
    fn canonical_path_removes_export_wrapper() {
        assert_eq!(
            canonical_export_path("takeout-001/Takeout/Google Photos/2026/pic.jpg"),
            "Takeout/Google Photos/2026/pic.jpg"
        );
        assert_eq!(
            canonical_export_path(
                "account/your_instagram_activity/messages/inbox/a/message_1.json"
            ),
            "your_instagram_activity/messages/inbox/a/message_1.json"
        );
    }

    #[test]
    fn classifies_and_joins_paths() {
        assert_eq!(file_kind("a/photo.JPG"), FileKind::Media);
        assert_eq!(file_kind("a/data.json"), FileKind::Json);
        assert_eq!(file_kind("index.HTM"), FileKind::Html);
        assert_eq!(file_kind("notes.txt"), FileKind::Other);
        assert!(is_media("clip.mp4"));
        assert_eq!(extension("archive"), None);
        assert_eq!(parent("a/b.txt"), "a");
        assert_eq!(basename("a/b.txt"), "b.txt");
        assert_eq!(join("a", "b.txt"), "a/b.txt");
        assert_eq!(join("", "b.txt"), "b.txt");
    }
}
