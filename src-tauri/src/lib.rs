mod commands;
pub mod config;
pub mod db;
mod note_file;
pub mod store;
mod watcher;

use std::sync::Mutex;

use tauri::Manager;

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
                }
                watcher::restart(app.handle(), notes_root.clone());
            }
            app.manage(NotesRootState(Mutex::new(config.notes_root)));

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::db_health_check,
            commands::get_notes_root,
            commands::detect_cloud_folders,
            commands::set_notes_root,
            commands::list_folders,
            commands::list_notes,
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
