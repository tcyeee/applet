//! Filesystem layout for the runtime's app data directory:
//!
//! ```text
//! <base_dir>/
//!   registry.sqlite            -- App Registry (see registry.rs)
//!   apps/<app_id>/
//!     data.sqlite              -- the app's own database (see appdb.rs)
//!     files/                   -- the app's private file storage (see storage.rs)
//!   shared/                    -- storage shared across all apps (see storage.rs)
//!   backups/<app_id>/<timestamp>.zip
//!     manifest.json            -- full AppRecord snapshot (incl. definition)
//!     data.sqlite
//!     files/...
//! ```

use std::path::{Path, PathBuf};

pub fn registry_db_path(base_dir: &Path) -> PathBuf {
    base_dir.join("registry.sqlite")
}

pub fn apps_dir(base_dir: &Path) -> PathBuf {
    base_dir.join("apps")
}

pub fn app_dir(base_dir: &Path, app_id: &str) -> PathBuf {
    apps_dir(base_dir).join(app_id)
}

pub fn app_db_path(base_dir: &Path, app_id: &str) -> PathBuf {
    app_dir(base_dir, app_id).join("data.sqlite")
}

pub fn app_files_dir(base_dir: &Path, app_id: &str) -> PathBuf {
    app_dir(base_dir, app_id).join("files")
}

pub fn shared_dir(base_dir: &Path) -> PathBuf {
    base_dir.join("shared")
}

pub fn backups_dir(base_dir: &Path, app_id: &str) -> PathBuf {
    base_dir.join("backups").join(app_id)
}
