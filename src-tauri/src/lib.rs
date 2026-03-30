mod commands;
mod core;

use std::sync::Arc;

use core::cancel_token::CancelToken;
use core::skill_store::{default_db_path, migrate_legacy_db_if_needed, SkillStore};
use tauri::menu::{Menu, MenuItem, Submenu};
use tauri::tray::TrayIconBuilder;
use tauri::Manager;
use tauri_plugin_log::{Target, TargetKind};

struct MenuLabels {
    features: &'static str,
    open_skills_path: &'static str,
    show_main: &'static str,
    quit: &'static str,
}

fn menu_labels() -> MenuLabels {
    menu_labels_from(None)
}

fn menu_labels_from(language: Option<&str>) -> MenuLabels {
    if let Some(lang) = language {
        let lower = lang.to_lowercase();
        if lower.starts_with("zh") {
            return MenuLabels {
                features: "功能",
                open_skills_path: "打开Skills存储路径",
                show_main: "显示主窗口",
                quit: "退出",
            };
        }
    }
    let locale = std::env::var("LC_ALL")
        .or_else(|_| std::env::var("LANG"))
        .unwrap_or_default()
        .to_lowercase();
    let is_zh = locale.starts_with("zh") || locale.contains("_zh") || locale.contains(".zh");
    if is_zh {
        MenuLabels {
            features: "功能",
            open_skills_path: "打开Skills存储路径",
            show_main: "显示主窗口",
            quit: "退出",
        }
    } else {
        MenuLabels {
            features: "Features",
            open_skills_path: "Open Skills Storage Path",
            show_main: "Show Main Window",
            quit: "Quit",
        }
    }
}

fn build_app_menu(
    app: &tauri::AppHandle,
    labels: &MenuLabels,
) -> Result<Menu<tauri::Wry>, tauri::Error> {
    let app_open_repo_item = MenuItem::with_id(
        app,
        "app_open_central_repo",
        labels.open_skills_path,
        true,
        None::<&str>,
    )?;
    let quick_actions_submenu =
        Submenu::with_items(app, labels.features, true, &[&app_open_repo_item])?;
    let app_menu = Menu::default(app)?;
    app_menu.append(&quick_actions_submenu)?;
    Ok(app_menu)
}

fn build_tray_menu(
    app: &tauri::AppHandle,
    labels: &MenuLabels,
) -> Result<Menu<tauri::Wry>, tauri::Error> {
    let open_repo_item = MenuItem::with_id(
        app,
        "tray_open_central_repo",
        labels.open_skills_path,
        true,
        None::<&str>,
    )?;
    let show_item = MenuItem::with_id(app, "tray_show_main", labels.show_main, true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "tray_quit", labels.quit, true, None::<&str>)?;
    Menu::with_items(app, &[&open_repo_item, &show_item, &quit_item])
}

pub(crate) fn update_native_menu_language(
    app: &tauri::AppHandle,
    language: Option<&str>,
) -> Result<(), String> {
    let labels = menu_labels_from(language);
    let app_menu = build_app_menu(app, &labels).map_err(|err| err.to_string())?;
    app.set_menu(app_menu).map_err(|err| err.to_string())?;
    if let Some(tray) = app.tray_by_id("main-tray") {
        let tray_menu = build_tray_menu(app, &labels).map_err(|err| err.to_string())?;
        tray.set_menu(Some(tray_menu))
            .map_err(|err| err.to_string())?;
    }
    Ok(())
}

fn open_dir_in_file_manager(path: &std::path::Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let mut cmd = std::process::Command::new("open");
    #[cfg(target_os = "windows")]
    let mut cmd = std::process::Command::new("explorer");
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut cmd = std::process::Command::new("xdg-open");

    let status = cmd
        .arg(path)
        .status()
        .map_err(|err| format!("failed to launch file manager: {}", err))?;
    if !status.success() {
        return Err(format!("file manager exited with status {}", status));
    }
    Ok(())
}

fn open_central_repo_dir<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<(), String> {
    let store = app.state::<SkillStore>().inner().clone();
    let path = core::central_repo::resolve_central_repo_path(app, &store)
        .and_then(|path| {
            core::central_repo::ensure_central_repo(&path)?;
            Ok(path)
        })
        .map_err(|err| err.to_string())?;
    open_dir_in_file_manager(&path)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            app.handle().plugin(
                tauri_plugin_log::Builder::default()
                    .level(log::LevelFilter::Info)
                    .targets([
                        Target::new(TargetKind::LogDir { file_name: None }),
                        #[cfg(desktop)]
                        Target::new(TargetKind::Stdout),
                    ])
                    .build(),
            )?;

            let db_path = default_db_path(app.handle()).map_err(tauri::Error::from)?;
            migrate_legacy_db_if_needed(&db_path).map_err(tauri::Error::from)?;
            let store = SkillStore::new(db_path);
            store.ensure_schema().map_err(tauri::Error::from)?;
            app.manage(store.clone());
            app.manage(Arc::new(CancelToken::new()));
            let labels = menu_labels();
            let app_menu = build_app_menu(app.handle(), &labels)?;
            app.set_menu(app_menu)?;
            app.on_menu_event(|app, event| {
                if event.id().as_ref() == "app_open_central_repo" {
                    if let Err(err) = open_central_repo_dir(app) {
                        log::error!("failed to open central repo from app menu: {}", err);
                    }
                }
            });
            let tray_menu = build_tray_menu(app.handle(), &labels)?;
            let mut tray_builder = TrayIconBuilder::with_id("main-tray")
                .menu(&tray_menu)
                .show_menu_on_left_click(true)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "tray_open_central_repo" => {
                        if let Err(err) = open_central_repo_dir(app) {
                            log::error!("failed to open central repo from tray: {}", err);
                        }
                    }
                    "tray_show_main" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "tray_quit" => {
                        app.exit(0);
                    }
                    _ => {}
                });
            if let Some(default_icon) = app.default_window_icon() {
                tray_builder = tray_builder.icon(default_icon.clone());
            }
            tray_builder.build(app)?;

            // Backfill description for skills that were installed before V2 schema.
            core::installer::backfill_skill_descriptions(&store);

            // Best-effort cleanup of our own old git temp directories.
            // Safety:
            // - Only deletes directories that match prefix `skillverse-git-*`
            // - And contain our marker file `.skillverse-git-temp`
            // - And are older than the max age.
            let handle = app.handle().clone();
            let store_for_cleanup = store.clone();
            tauri::async_runtime::spawn(async move {
                let removed = core::temp_cleanup::cleanup_old_git_temp_dirs(
                    &handle,
                    std::time::Duration::from_secs(24 * 60 * 60),
                )
                .unwrap_or(0);
                if removed > 0 {
                    log::info!("cleaned up {} old git temp dirs", removed);
                }

                let cleanup_days =
                    core::cache_cleanup::get_git_cache_cleanup_days(&store_for_cleanup);
                if cleanup_days > 0 {
                    let max_age =
                        std::time::Duration::from_secs(cleanup_days as u64 * 24 * 60 * 60);
                    let removed =
                        core::cache_cleanup::cleanup_git_cache_dirs(&handle, max_age).unwrap_or(0);
                    if removed > 0 {
                        log::info!("cleaned up {} git cache dirs", removed);
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_central_repo_path,
            commands::open_central_repo_folder,
            commands::set_ui_language,
            commands::set_central_repo_path,
            commands::get_tool_status,
            commands::get_git_cache_cleanup_days,
            commands::get_git_cache_ttl_secs,
            commands::set_git_cache_cleanup_days,
            commands::set_git_cache_ttl_secs,
            commands::clear_git_cache_now,
            commands::get_onboarding_plan,
            commands::install_local,
            commands::list_local_skills_cmd,
            commands::install_local_selection,
            commands::install_git,
            commands::list_git_skills_cmd,
            commands::install_git_selection,
            commands::sync_skill_dir,
            commands::sync_skill_to_tool,
            commands::unsync_skill_from_tool,
            commands::update_managed_skill,
            commands::search_github,
            commands::get_github_token,
            commands::set_github_token,
            commands::import_existing_skill,
            commands::get_managed_skills,
            commands::delete_managed_skill,
            commands::get_featured_skills,
            commands::search_skills_online,
            commands::list_skill_files,
            commands::read_skill_file,
            commands::cancel_current_operation
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
