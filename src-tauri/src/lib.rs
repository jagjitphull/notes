mod commands;
pub mod config;
pub mod db;
mod note_file;
pub mod store;
mod watcher;

use std::sync::Mutex;

use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager, WindowEvent,
};

use commands::NotesRootState;
use watcher::WatcherState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
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
                    match store::purge_expired_trash(&conn, &notes_root, store::TRASH_RETENTION_DAYS) {
                        Ok(n) if n > 0 => log::info!("purged {n} note(s) past the trash retention window"),
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
            commands::list_tags,
            commands::get_note_body,
            commands::save_note_body,
            commands::create_note,
            commands::set_note_pinned,
            commands::set_note_deleted,
            commands::delete_note_permanently,
            commands::move_note,
            commands::create_folder,
            commands::rename_folder,
            commands::delete_folder,
        ])
        .run(tauri::generate_context!())
        .expect("error while building tauri application");
}
