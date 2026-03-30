use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub enum SyncMode {
    Auto,
    Symlink,
    Junction,
    Copy,
}

#[derive(Clone, Debug)]
pub struct SyncOutcome {
    pub mode_used: SyncMode,
    pub target_path: PathBuf,
    pub replaced: bool,
}

pub fn sync_dir_hybrid(source: &Path, target: &Path) -> Result<SyncOutcome> {
    if target.exists() {
        if is_same_link(target, source) {
            return Ok(SyncOutcome {
                mode_used: SyncMode::Symlink,
                target_path: target.to_path_buf(),
                replaced: false,
            });
        }
        anyhow::bail!("target already exists: {:?}", target);
    }

    ensure_parent_dir(target)?;

    if try_link_dir(source, target).is_ok() {
        return Ok(SyncOutcome {
            mode_used: SyncMode::Symlink,
            target_path: target.to_path_buf(),
            replaced: false,
        });
    }

    #[cfg(windows)]
    if try_junction(source, target).is_ok() {
        return Ok(SyncOutcome {
            mode_used: SyncMode::Junction,
            target_path: target.to_path_buf(),
            replaced: false,
        });
    }

    copy_dir_recursive(source, target)?;
    Ok(SyncOutcome {
        mode_used: SyncMode::Copy,
        target_path: target.to_path_buf(),
        replaced: false,
    })
}

pub fn sync_dir_hybrid_with_overwrite(
    source: &Path,
    target: &Path,
    overwrite: bool,
) -> Result<SyncOutcome> {
    let mut did_replace = false;
    if std::fs::symlink_metadata(target).is_ok() {
        if is_same_link(target, source) {
            return Ok(SyncOutcome {
                mode_used: SyncMode::Symlink,
                target_path: target.to_path_buf(),
                replaced: false,
            });
        }

        if overwrite {
            std::fs::remove_dir_all(target)
                .with_context(|| format!("remove existing target {:?}", target))?;
            did_replace = true;
        } else {
            anyhow::bail!("target already exists: {:?}", target);
        }
    }

    // reuse normal flow
    sync_dir_hybrid(source, target).map(|mut out| {
        out.replaced = did_replace;
        out
    })
}

pub fn sync_dir_copy_with_overwrite(
    source: &Path,
    target: &Path,
    overwrite: bool,
) -> Result<SyncOutcome> {
    let mut did_replace = false;
    if std::fs::symlink_metadata(target).is_ok() {
        if overwrite {
            remove_path_any(target)
                .with_context(|| format!("remove existing target {:?}", target))?;
            did_replace = true;
        } else {
            anyhow::bail!("target already exists: {:?}", target);
        }
    }

    ensure_parent_dir(target)?;
    copy_dir_recursive(source, target)?;

    Ok(SyncOutcome {
        mode_used: SyncMode::Copy,
        target_path: target.to_path_buf(),
        replaced: did_replace,
    })
}

pub fn sync_dir_for_tool_with_overwrite(
    tool_key: &str,
    source: &Path,
    target: &Path,
    overwrite: bool,
) -> Result<SyncOutcome> {
    // Cursor 目前不支持软链/junction：强制使用 copy，避免同步后在 Cursor 内不可用。
    if tool_key.eq_ignore_ascii_case("cursor") {
        return sync_dir_copy_with_overwrite(source, target, overwrite);
    }
    sync_dir_hybrid_with_overwrite(source, target, overwrite)
}

fn ensure_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("create dir {:?}", parent))?;
    }
    Ok(())
}

fn remove_path_any(path: &Path) -> Result<()> {
    let meta = match std::fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err).with_context(|| format!("stat {:?}", path)),
    };
    let ft = meta.file_type();

    // 软链接（即使指向目录）也应该用 remove_file 删除链接本身
    if ft.is_symlink() {
        std::fs::remove_file(path).with_context(|| format!("remove symlink {:?}", path))?;
        return Ok(());
    }
    if ft.is_dir() {
        std::fs::remove_dir_all(path).with_context(|| format!("remove dir {:?}", path))?;
        return Ok(());
    }
    std::fs::remove_file(path).with_context(|| format!("remove file {:?}", path))?;
    Ok(())
}

fn is_same_link(link_path: &Path, target: &Path) -> bool {
    if let Ok(existing) = std::fs::read_link(link_path) {
        return existing == target;
    }
    false
}

fn try_link_dir(source: &Path, target: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(source, target)
            .with_context(|| format!("symlink {:?} -> {:?}", target, source))?;
        Ok(())
    }

    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_dir(source, target)
            .with_context(|| format!("symlink {:?} -> {:?}", target, source))?;
        return Ok(());
    }

    #[cfg(not(any(unix, windows)))]
    anyhow::bail!("symlink not supported on this platform");
}

#[cfg(windows)]
fn try_junction(source: &Path, target: &Path) -> Result<()> {
    junction::create(source, target)
        .with_context(|| format!("junction {:?} -> {:?}", target, source))?;
    Ok(())
}

fn should_skip_copy(entry: &walkdir::DirEntry) -> bool {
    entry.file_name() == ".git"
}

pub fn copy_dir_recursive(source: &Path, target: &Path) -> Result<()> {
    let profile = std::env::var("SKILLVERSE_PROFILE_IO")
        .ok()
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let started = std::time::Instant::now();
    let mut copied_files: u64 = 0;
    let mut copied_bytes: u64 = 0;

    for entry in walkdir::WalkDir::new(source)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| !should_skip_copy(entry))
    {
        let entry = entry?;
        if should_skip_copy(&entry) {
            continue;
        }
        let relative = entry.path().strip_prefix(source)?;
        let target_path = target.join(relative);

        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target_path)
                .with_context(|| format!("create dir {:?}", target_path))?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = target_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let bytes = std::fs::copy(entry.path(), &target_path)
                .with_context(|| format!("copy file {:?} -> {:?}", entry.path(), target_path))?;
            if profile {
                copied_files += 1;
                copied_bytes = copied_bytes.saturating_add(bytes);
            }
        }
    }
    if profile {
        log::info!(
            "[sync_engine] copy_dir_recursive {} files, {} bytes in {}s (src={:?} dst={:?})",
            copied_files,
            copied_bytes,
            started.elapsed().as_secs_f32(),
            source,
            target
        );
    }
    Ok(())
}

const MAX_LOCAL_SYMLINK_DEPTH: u32 = 64;

/// Copy a directory tree into `target`, resolving in-repo symlinks to plain files/dirs.
/// Only symlink targets under `repo_root` are allowed.
pub fn copy_dir_recursive_materialize_symlinks(
    source: &Path,
    target: &Path,
    repo_root: &Path,
) -> Result<()> {
    let root_canon = repo_root
        .canonicalize()
        .with_context(|| format!("canonicalize repo root {:?}", repo_root))?;
    copy_dir_recursive_materialize_inner(source, target, &root_canon, 0)
}

fn copy_dir_recursive_materialize_inner(
    source: &Path,
    target: &Path,
    root_canon: &Path,
    symlink_depth: u32,
) -> Result<()> {
    if symlink_depth > MAX_LOCAL_SYMLINK_DEPTH {
        anyhow::bail!(
            "local symlink materialization depth exceeded (max {}) under {:?}",
            MAX_LOCAL_SYMLINK_DEPTH,
            source
        );
    }

    for entry in walkdir::WalkDir::new(source)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| !should_skip_copy(entry))
    {
        let entry = entry?;
        if should_skip_copy(&entry) {
            continue;
        }
        let relative = entry.path().strip_prefix(source)?;
        let target_path = target.join(relative);
        let ft = entry.file_type();

        if ft.is_dir() {
            std::fs::create_dir_all(&target_path)
                .with_context(|| format!("create dir {:?}", target_path))?;
        } else if ft.is_file() {
            // Some git clients (or configs like core.symlinks=false) materialize symlink blobs
            // as tiny text files containing a relative path. Detect and materialize them.
            if let Some(pointer_target) = resolve_pointer_file_target(entry.path(), root_canon)
                .with_context(|| format!("resolve pointer-like symlink file {:?}", entry.path()))?
            {
                let pointer_meta = std::fs::symlink_metadata(&pointer_target)
                    .with_context(|| format!("stat pointer target {:?}", pointer_target))?;
                if pointer_meta.is_dir() {
                    std::fs::create_dir_all(&target_path)
                        .with_context(|| format!("create dir {:?}", target_path))?;
                    copy_dir_recursive_materialize_inner(
                        &pointer_target,
                        &target_path,
                        root_canon,
                        symlink_depth + 1,
                    )?;
                } else {
                    if let Some(parent) = target_path.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    std::fs::copy(&pointer_target, &target_path).with_context(|| {
                        format!(
                            "copy pointer target {:?} -> {:?}",
                            pointer_target, target_path
                        )
                    })?;
                }
            } else {
                if let Some(parent) = target_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::copy(entry.path(), &target_path).with_context(|| {
                    format!("copy file {:?} -> {:?}", entry.path(), target_path)
                })?;
            }
        } else if ft.is_symlink() {
            let link_target = std::fs::read_link(entry.path())
                .with_context(|| format!("read_link {:?}", entry.path()))?;
            let resolved = entry.path().parent().unwrap_or(source).join(&link_target);
            let resolved_canon = resolved.canonicalize().with_context(|| {
                format!(
                    "canonicalize symlink target {:?} (from {:?})",
                    resolved,
                    entry.path()
                )
            })?;

            if !resolved_canon.starts_with(root_canon) {
                anyhow::bail!(
                    "symlink {:?} resolves outside repository root: {:?}",
                    entry.path(),
                    resolved_canon
                );
            }

            let meta = std::fs::symlink_metadata(&resolved_canon)
                .with_context(|| format!("stat {:?}", resolved_canon))?;
            if meta.is_dir() {
                std::fs::create_dir_all(&target_path)
                    .with_context(|| format!("create dir {:?}", target_path))?;
                copy_dir_recursive_materialize_inner(
                    &resolved_canon,
                    &target_path,
                    root_canon,
                    symlink_depth + 1,
                )?;
            } else {
                if let Some(parent) = target_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::copy(&resolved_canon, &target_path).with_context(|| {
                    format!(
                        "copy symlink target {:?} -> {:?}",
                        resolved_canon, target_path
                    )
                })?;
            }
        }
    }
    Ok(())
}

fn resolve_pointer_file_target(file_path: &Path, root_canon: &Path) -> Result<Option<PathBuf>> {
    const MAX_POINTER_LEN: u64 = 512;
    let meta = std::fs::metadata(file_path).with_context(|| format!("stat {:?}", file_path))?;
    if meta.len() == 0 || meta.len() > MAX_POINTER_LEN {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(file_path)
        .with_context(|| format!("read pointer-like file {:?}", file_path))?;
    let trimmed = raw.trim();
    if trimmed.is_empty() || (!trimmed.starts_with("./") && !trimmed.starts_with("../")) {
        return Ok(None);
    }
    if trimmed.contains('\0') || trimmed.contains('\n') || trimmed.contains('\r') {
        return Ok(None);
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/'))
    {
        return Ok(None);
    }
    let resolved = file_path.parent().unwrap_or(root_canon).join(trimmed);
    let canon = match resolved.canonicalize() {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    if !canon.starts_with(root_canon) {
        anyhow::bail!(
            "pointer file {:?} resolves outside repo: {:?}",
            file_path,
            canon
        );
    }
    if !canon.exists() {
        return Ok(None);
    }
    Ok(Some(canon))
}

#[cfg(test)]
#[path = "tests/sync_engine.rs"]
mod tests;
