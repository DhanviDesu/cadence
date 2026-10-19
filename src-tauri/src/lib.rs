mod commands;
mod db;
mod domain;
mod export;
mod import;
mod parse;

use std::sync::Mutex;

use commands::DbState;
use rusqlite::Connection;
use tauri::{Manager, WebviewWindow};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

/// ⌃⌥N - chosen specifically to avoid system/app collisions: ⌘⇧G was
/// considered first but clashes with Finder's "Go to Folder" and most apps'
/// "Find Previous".
fn quickadd_shortcut() -> Shortcut {
    Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyN)
}

/// ⌃⌥D - the "preview today" popup. Same low-collision pattern as quickadd.
fn preview_shortcut() -> Shortcut {
    Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyD)
}

fn toggle_window(window: &WebviewWindow) {
    let visible = window.is_visible().unwrap_or(false);
    if visible {
        let _ = window.hide();
    } else {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state() != ShortcutState::Pressed {
                        return;
                    }
                    let label = if shortcut == &quickadd_shortcut() {
                        Some("quickadd")
                    } else if shortcut == &preview_shortcut() {
                        Some("preview")
                    } else {
                        None
                    };
                    if let Some(window) = label.and_then(|l| app.get_webview_window(l)) {
                        toggle_window(&window);
                    }
                })
                .build(),
        )
        .setup(|app| {
            let data_dir = app.path().app_data_dir().expect("no app data dir");
            std::fs::create_dir_all(&data_dir).expect("failed to create app data dir");
            let db_path = data_dir.join("cadence.db");
            let conn = Connection::open(db_path).expect("failed to open database");
            db::migrations::init(&conn).expect("failed to run migrations");
            app.manage(DbState(Mutex::new(conn)));

            app.global_shortcut().register(quickadd_shortcut())?;
            app.global_shortcut().register(preview_shortcut())?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_today,
            commands::create_task,
            commands::update_task_title,
            commands::set_status,
            commands::clear_status_override,
            commands::toggle_done,
            commands::delete_task,
            commands::indent_task,
            commands::outdent_task,
            commands::move_task,
            commands::list_routines,
            commands::create_routine,
            commands::delete_routine,
            commands::add_routine_item,
            commands::update_routine_item,
            commands::delete_routine_item,
            commands::move_routine,
            commands::move_routine_item,
            commands::set_routine_after_tasks,
            commands::get_day_by_date,
            commands::import_note,
            commands::export_all_days,
            commands::search_tasks,
            commands::summarize_range,
            commands::get_backlog,
            commands::pull_from_backlog,
            commands::set_backlog_snooze,
            commands::drop_from_backlog,
            commands::get_close_out_candidates,
            commands::run_close_out,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
