//! The native application menu bar (File/Edit/View/Window/Help, plus a
//! macOS app submenu) - set once at startup via `app.set_menu` in
//! `lib.rs`'s setup hook, alongside the existing tray menu (a separate,
//! independent menu attached to the tray icon, not this one).
//!
//! Predefined items (Undo, Copy, Quit, ...) are handled natively by the
//! OS/webview with no Rust-side wiring needed. Custom items just forward
//! their id to the frontend as a "menu" event (see `handle_event` below);
//! App.vue's listener routes most of those ids straight through its
//! existing `paletteActions` registry - the same one the command palette
//! uses - so a menu click, a keyboard shortcut, and a command-palette
//! selection all end up calling the exact same handler.

use tauri::{
    AppHandle, Emitter, Wry,
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu},
};

fn file_menu(app: &AppHandle) -> tauri::Result<Submenu<Wry>> {
    let new_note = MenuItem::with_id(app, "new-note", "New Note", true, Some("CmdOrCtrl+N"))?;
    let new_folder = MenuItem::with_id(app, "new-folder", "New Folder", true, None::<&str>)?;
    let import = MenuItem::with_id(
        app,
        "import-markdown",
        "Import Markdown Folder…",
        true,
        None::<&str>,
    )?;
    let backup = MenuItem::with_id(
        app,
        "export-backup",
        "Backup All Notes…",
        true,
        None::<&str>,
    )?;
    let restore = MenuItem::with_id(
        app,
        "restore-backup",
        "Restore from Backup…",
        true,
        None::<&str>,
    )?;

    // Quit lives in the macOS app submenu (app_menu below) instead of
    // here, matching the platform convention every other mac app follows.
    #[cfg(target_os = "macos")]
    {
        Submenu::with_items(
            app,
            "File",
            true,
            &[
                &new_note,
                &new_folder,
                &PredefinedMenuItem::separator(app)?,
                &import,
                &backup,
                &restore,
            ],
        )
    }
    #[cfg(not(target_os = "macos"))]
    {
        let quit = PredefinedMenuItem::quit(app, None)?;
        Submenu::with_items(
            app,
            "File",
            true,
            &[
                &new_note,
                &new_folder,
                &PredefinedMenuItem::separator(app)?,
                &import,
                &backup,
                &restore,
                &PredefinedMenuItem::separator(app)?,
                &quit,
            ],
        )
    }
}

fn edit_menu(app: &AppHandle) -> tauri::Result<Submenu<Wry>> {
    let find = MenuItem::with_id(app, "find", "Find", true, Some("CmdOrCtrl+F"))?;
    Submenu::with_items(
        app,
        "Edit",
        true,
        &[
            &PredefinedMenuItem::undo(app, None)?,
            &PredefinedMenuItem::redo(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::cut(app, None)?,
            &PredefinedMenuItem::copy(app, None)?,
            &PredefinedMenuItem::paste(app, None)?,
            &PredefinedMenuItem::select_all(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &find,
        ],
    )
}

fn view_menu(app: &AppHandle) -> tauri::Result<Submenu<Wry>> {
    let command_palette = MenuItem::with_id(
        app,
        "command-palette",
        "Command Palette…",
        true,
        Some("CmdOrCtrl+K"),
    )?;
    let today = MenuItem::with_id(app, "today", "Today", true, Some("CmdOrCtrl+T"))?;
    let focus_mode = MenuItem::with_id(
        app,
        "toggle-focus-mode",
        "Toggle Focus Mode",
        true,
        Some("CmdOrCtrl+."),
    )?;
    Submenu::with_items(app, "View", true, &[&command_palette, &today, &focus_mode])
}

fn window_menu(app: &AppHandle) -> tauri::Result<Submenu<Wry>> {
    Submenu::with_items(
        app,
        "Window",
        true,
        &[
            &PredefinedMenuItem::minimize(app, None)?,
            &PredefinedMenuItem::close_window(app, None)?,
        ],
    )
}

fn help_menu(app: &AppHandle) -> tauri::Result<Submenu<Wry>> {
    let about = MenuItem::with_id(app, "about", "About Notes", true, None::<&str>)?;
    let github = MenuItem::with_id(app, "github", "View on GitHub", true, None::<&str>)?;
    Submenu::with_items(app, "Help", true, &[&about, &github])
}

#[cfg(target_os = "macos")]
fn mac_app_menu(app: &AppHandle) -> tauri::Result<Submenu<Wry>> {
    let about = MenuItem::with_id(app, "about", "About Notes", true, None::<&str>)?;
    Submenu::with_items(
        app,
        "Notes",
        true,
        &[
            &about,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::hide(app, None)?,
            &PredefinedMenuItem::hide_others(app, None)?,
            &PredefinedMenuItem::show_all(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::quit(app, None)?,
        ],
    )
}

pub fn build(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    #[cfg(target_os = "macos")]
    {
        Menu::with_items(
            app,
            &[
                &mac_app_menu(app)?,
                &file_menu(app)?,
                &edit_menu(app)?,
                &view_menu(app)?,
                &window_menu(app)?,
                &help_menu(app)?,
            ],
        )
    }
    #[cfg(not(target_os = "macos"))]
    {
        Menu::with_items(
            app,
            &[
                &file_menu(app)?,
                &edit_menu(app)?,
                &view_menu(app)?,
                &window_menu(app)?,
                &help_menu(app)?,
            ],
        )
    }
}

pub fn handle_event(app: &AppHandle, event: MenuEvent) {
    let _ = app.emit("menu", event.id.as_ref());
}
