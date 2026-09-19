//! File storage: each app gets a private directory it cannot escape, plus a
//! `shared/` directory reachable by every app.
//!
//! Isolation is structural, not permission-based: the Tauri commands built on
//! top of this module only ever pass an app's *own* `app_files_dir`, so one
//! app has no code path to another app's private files. Per-app *granted*
//! access to the shared directory (the "用户可见的权限授权 UI" in TODO step 2)
//! is future work that needs a `permissions` field on the App Schema, which
//! doesn't exist yet — today the shared directory is open to every app.

use std::fmt;
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

#[derive(Debug)]
pub enum StorageError {
    PathEscape(String),
    Io(std::io::Error),
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StorageError::PathEscape(p) => write!(f, "path \"{p}\" escapes the storage root"),
            StorageError::Io(e) => write!(f, "storage io error: {e}"),
        }
    }
}

impl std::error::Error for StorageError {}
impl From<std::io::Error> for StorageError {
    fn from(e: std::io::Error) -> Self {
        StorageError::Io(e)
    }
}

/// Resolves `relative` against `root`, refusing absolute paths, empty paths,
/// and any `..` component that would climb out of `root` — the only path
/// traversal guard here, since SQLite identifier concerns don't apply to
/// plain filesystem paths.
fn resolve(root: &Path, relative: &str) -> Result<PathBuf, StorageError> {
    if relative.is_empty() {
        return Err(StorageError::PathEscape(relative.to_string()));
    }
    let rel_path = Path::new(relative);
    if rel_path.is_absolute() {
        return Err(StorageError::PathEscape(relative.to_string()));
    }
    if rel_path
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(StorageError::PathEscape(relative.to_string()));
    }
    Ok(root.join(rel_path))
}

pub fn write_file(root: &Path, relative: &str, contents: &[u8]) -> Result<(), StorageError> {
    let path = resolve(root, relative)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, contents)?;
    Ok(())
}

pub fn read_file(root: &Path, relative: &str) -> Result<Vec<u8>, StorageError> {
    let path = resolve(root, relative)?;
    Ok(std::fs::read(path)?)
}

pub fn delete_file(root: &Path, relative: &str) -> Result<(), StorageError> {
    let path = resolve(root, relative)?;
    if path.is_dir() {
        std::fs::remove_dir_all(path)?;
    } else {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

/// Lists all files under `root` as slash-separated paths relative to `root`.
pub fn list_files(root: &Path) -> Result<Vec<String>, StorageError> {
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in WalkDir::new(root).into_iter().filter_map(Result::ok) {
        if entry.file_type().is_file() {
            let rel = entry.path().strip_prefix(root).unwrap_or(entry.path());
            out.push(
                rel.to_string_lossy()
                    .replace(std::path::MAIN_SEPARATOR, "/"),
            );
        }
    }
    out.sort();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_read_delete_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write_file(root, "notes/a.txt", b"hello").unwrap();
        assert_eq!(read_file(root, "notes/a.txt").unwrap(), b"hello");
        assert_eq!(list_files(root).unwrap(), vec!["notes/a.txt".to_string()]);
        delete_file(root, "notes/a.txt").unwrap();
        assert!(read_file(root, "notes/a.txt").is_err());
    }

    #[test]
    fn rejects_parent_dir_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let err = write_file(root, "../escape.txt", b"x").unwrap_err();
        assert!(matches!(err, StorageError::PathEscape(_)));
    }

    #[test]
    fn rejects_absolute_path() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let err = write_file(root, "/etc/passwd", b"x").unwrap_err();
        assert!(matches!(err, StorageError::PathEscape(_)));
    }

    #[test]
    fn apps_with_different_roots_cannot_see_each_others_files() {
        let base = tempfile::tempdir().unwrap();
        let app_a = base.path().join("apps/a/files");
        let app_b = base.path().join("apps/b/files");
        write_file(&app_a, "secret.txt", b"a-only").unwrap();
        assert!(list_files(&app_b).unwrap().is_empty());
        assert!(read_file(&app_b, "secret.txt").is_err());
    }
}
