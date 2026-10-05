mod commands;
pub mod config;
pub mod db;
mod embeddings;
mod note_file;
pub mod store;
mod versions;
mod watcher;

use std::sync::Mutex;

use tauri::{
    Emitter, Manager, WindowEvent,
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
};

use commands::NotesRootState;
use watcher::WatcherState;

// Chosen to mirror Obsidian/Bear-style "quick capture" hotkeys. Known
// collision: this is also Chrome's "New incognito window" shortcut on
// Windows/Linux, so pressing it while Chrome is the focused app opens an
// incognito window instead of reaching this app - registering a global
// shortcut can't detect or avoid that, it only wins when no other app has
// already claimed the combination first.
const QUICK_CAPTURE_SHORTCUT: &str = "CmdOrCtrl+Shift+N";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();

    // Must be registered before any other plugin (Tauri's own
    // requirement): a second launch is redirected here instead of
    // starting a second process, which matters for this app specifically
    // because two processes would both open the same SQLite index and
    // watch the same notes folder, racing each other. Desktop-only API,
    // hence the cfg guard (this app has no mobile target anyway).
    #[cfg(desktop)]
    {
        builder = builder
            .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }))
            .plugin(tauri_plugin_updater::Builder::new().build())
            .plugin(tauri_plugin_process::init())
            .plugin(
                tauri_plugin_global_shortcut::Builder::new()
                    .with_shortcut(QUICK_CAPTURE_SHORTCUT)
                    .expect("QUICK_CAPTURE_SHORTCUT is a valid accelerator string")
                    .with_handler(|app, _shortcut, event| {
                        if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                            // The frontend owns note creation (same command
                            // path as the "+" button) so it can land the new
                            // note in the right list and focus the editor;
                            // this just asks it to do that.
                            let _ = app.emit("quick-capture", ());
                        }
                    })
                    .build(),
            );
    }

    builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("failed to resolve app data directory");
            let conn = db::init(&app_data_dir).expect("failed to initialize database");
            app.manage(db::DbState(Mutex::new(conn)));
            app.manage(WatcherState(Mutex::new(None)));
            app.manage(commands::LoadedHashState(Mutex::new(
                std::collections::HashMap::new(),
            )));

            let app_config_dir = app
                .path()
                .app_config_dir()
                .expect("failed to resolve app config directory");
            let config = config::load(&app_config_dir);

            if let Some(notes_root) = config.notes_root.clone() {
                let db_state = app.state::<db::DbState>();
                if let Ok(conn) = db_state.0.lock() {
                    if let Err(e) = store::full_rescan(&conn, &notes_root) {
                        log::warn!("initial notes rescan failed: {e}");
                    }
                    match store::purge_expired_trash(
                        &conn,
                        &notes_root,
                        store::TRASH_RETENTION_DAYS,
                    ) {
                        Ok(n) if n > 0 => {
                            log::info!("purged {n} note(s) past the trash retention window")
                        }
                        Ok(_) => {}
                        Err(e) => log::warn!("trash purge failed: {e}"),
                    }
                }
                watcher::restart(app.handle(), notes_root.clone());
            }
            app.manage(NotesRootState(Mutex::new(config.notes_root)));

            // System tray: lets the app keep running (and its file watcher
            // keep syncing) after the window is closed, same as most tray
            // apps. Left/right-click distinction is unsupported on Linux
            // trays, so this is deliberately just a menu, not a click
            // handler — that's also how e.g. Dropbox's Linux tray works.
            let show_item = MenuItem::with_id(app, "show", "Show Notes", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let tray_menu = Menu::with_items(app, &[&show_item, &quit_item])?;

            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("Notes")
                .menu(&tray_menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;

            // Closing the window hides it instead of quitting, so the app
            // (and its background file watcher) keeps running in the tray;
            // "Quit" from the tray menu is the only way to actually exit.
            if let Some(window) = app.get_webview_window("main") {
                let window_handle = window.clone();
                window.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = window_handle.hide();
                    }
                });

                // Some window managers don't apply tauri.conf.json's
                // configured size to an undecorated window (decorations are
                // off - App.vue draws its own title bar) until a real
                // resize happens; forcing one here on startup is a no-op
                // where the configured size already took, and the fix
                // where it didn't.
                let _ = window.set_size(tauri::LogicalSize::new(1200.0, 800.0));
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::db_health_check,
            commands::get_notes_root,
            commands::detect_cloud_folders,
            commands::set_notes_root,
            commands::list_folders,
            commands::list_notes,
            commands::search_notes,
            commands::smart_search_available,
            commands::smart_search,
            commands::list_tags,
            commands::get_note_body,
            commands::save_note_body,
            commands::list_note_versions,
            commands::restore_note_version,
            commands::export_file,
            commands::import_markdown_folder,
            commands::create_note,
            commands::create_note_from_template,
            commands::set_note_template,
            commands::get_or_create_daily_note,
            commands::set_note_pinned,
            commands::add_note_tag,
            commands::remove_note_tag,
            commands::set_note_deleted,
            commands::delete_note_permanently,
            commands::move_note,
            commands::save_attachment,
            commands::get_attachment_size,
            commands::open_attachment,
            commands::create_folder,
            commands::rename_folder,
            commands::delete_folder,
            commands::set_folder_color,
            commands::set_tag_color,
        ])
        .run(tauri::generate_context!())
        .expect("error while building tauri application");
}
