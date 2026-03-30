use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use dirs::home_dir;
use tauri::Manager;

use super::skill_store::SkillStore;

const CENTRAL_DIR_NAME: &str = ".skillverse";
const LEGACY_CENTRAL_DIR_NAME: &str = ".skillshub";

pub fn resolve_central_repo_path<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    store: &SkillStore,
) -> Result<PathBuf> {
    if let Some(path) = store.get_setting("central_repo_path")? {
        return Ok(PathBuf::from(path));
    }

    if let Some(home) = home_dir() {
        let new_path = home.join(CENTRAL_DIR_NAME);
        let legacy_path = home.join(LEGACY_CENTRAL_DIR_NAME);
        // Auto-migrate: if the old ~/.skillshub exists and new ~/.skillverse doesn't,
        // rename the directory in-place so existing skills are preserved.
        if legacy_path.exists()
            && !new_path.exists()
            && std::fs::rename(&legacy_path, &new_path).is_err()
        {
            // Cross-device or permission failure — fall back to the legacy path
            // so users don't lose their data.
            return Ok(legacy_path);
        }
        return Ok(new_path);
    }

    let base = app
        .path()
        .app_data_dir()
        .context("failed to resolve app data dir")?;
    Ok(base.join(CENTRAL_DIR_NAME))
}

pub fn ensure_central_repo(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path).with_context(|| format!("create {:?}", path))?;
    Ok(())
}

#[cfg(test)]
#[path = "tests/central_repo.rs"]
mod tests;
