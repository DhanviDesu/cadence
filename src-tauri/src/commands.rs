use std::collections::HashMap;
use std::sync::Mutex;

use rusqlite::Connection;
use tauri::State;

use crate::db::queries::{
    self, BacklogItem, CloseOutCandidate, CloseOutResult, DayPayload, Decision, ImportSummary, RangeSummary,
    RoutineDef, SearchResult,
};
use crate::export;

pub struct DbState(pub Mutex<Connection>);

fn today(conn: &Connection) -> Result<String, String> {
    queries::ensure_today(conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_today(db: State<DbState>) -> Result<DayPayload, String> {
    let conn = db.0.lock().unwrap();
    queries::get_today_payload(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_task(
    db: State<DbState>,
    parent_id: Option<i64>,
    after_id: Option<i64>,
    title: String,
) -> Result<DayPayload, String> {
    let conn = db.0.lock().unwrap();
    let date = today(&conn)?;
    queries::create_task(&conn, &date, parent_id, after_id, &title).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_task_title(db: State<DbState>, id: i64, title: String) -> Result<DayPayload, String> {
    let conn = db.0.lock().unwrap();
    let date = today(&conn)?;
    queries::update_task_title(&conn, &date, id, &title).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_status(db: State<DbState>, id: i64, status: String) -> Result<DayPayload, String> {
    let conn = db.0.lock().unwrap();
    let date = today(&conn)?;
    queries::set_status(&conn, &date, id, &status).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn clear_status_override(db: State<DbState>, id: i64) -> Result<DayPayload, String> {
    let conn = db.0.lock().unwrap();
    let date = today(&conn)?;
    queries::clear_status_override(&conn, &date, id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn toggle_done(db: State<DbState>, id: i64) -> Result<DayPayload, String> {
    let conn = db.0.lock().unwrap();
    let date = today(&conn)?;
    queries::toggle_done(&conn, &date, id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_task(db: State<DbState>, id: i64) -> Result<DayPayload, String> {
    let conn = db.0.lock().unwrap();
    let date = today(&conn)?;
    queries::delete_task(&conn, &date, id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn indent_task(db: State<DbState>, id: i64) -> Result<DayPayload, String> {
    let conn = db.0.lock().unwrap();
    let date = today(&conn)?;
    queries::indent_task(&conn, &date, id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn outdent_task(db: State<DbState>, id: i64) -> Result<DayPayload, String> {
    let conn = db.0.lock().unwrap();
    let date = today(&conn)?;
    queries::outdent_task(&conn, &date, id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn move_task(db: State<DbState>, id: i64, direction: String) -> Result<DayPayload, String> {
    let conn = db.0.lock().unwrap();
    let date = today(&conn)?;
    queries::move_task(&conn, &date, id, &direction).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_routines(db: State<DbState>) -> Result<Vec<RoutineDef>, String> {
    let conn = db.0.lock().unwrap();
    queries::list_routine_defs(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_routine(db: State<DbState>, name: String) -> Result<Vec<RoutineDef>, String> {
    let conn = db.0.lock().unwrap();
    queries::create_routine(&conn, &name).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_routine(db: State<DbState>, routine_id: i64) -> Result<Vec<RoutineDef>, String> {
    let conn = db.0.lock().unwrap();
    queries::delete_routine(&conn, routine_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn add_routine_item(db: State<DbState>, routine_id: i64, title: String) -> Result<Vec<RoutineDef>, String> {
    let conn = db.0.lock().unwrap();
    queries::add_routine_item(&conn, routine_id, &title).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_routine_item(db: State<DbState>, item_id: i64, title: String) -> Result<Vec<RoutineDef>, String> {
    let conn = db.0.lock().unwrap();
    queries::update_routine_item(&conn, item_id, &title).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_routine_item(db: State<DbState>, item_id: i64) -> Result<Vec<RoutineDef>, String> {
    let conn = db.0.lock().unwrap();
    queries::delete_routine_item(&conn, item_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn move_routine(db: State<DbState>, routine_id: i64, direction: String) -> Result<Vec<RoutineDef>, String> {
    let conn = db.0.lock().unwrap();
    queries::move_routine(&conn, routine_id, &direction).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn move_routine_item(db: State<DbState>, item_id: i64, direction: String) -> Result<Vec<RoutineDef>, String> {
    let conn = db.0.lock().unwrap();
    queries::move_routine_item(&conn, item_id, &direction).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_routine_after_tasks(
    db: State<DbState>,
    routine_id: i64,
    after_tasks: bool,
) -> Result<Vec<RoutineDef>, String> {
    let conn = db.0.lock().unwrap();
    queries::set_routine_after_tasks(&conn, routine_id, after_tasks).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_day_by_date(db: State<DbState>, date: String) -> Result<DayPayload, String> {
    let conn = db.0.lock().unwrap();
    queries::get_day(&conn, &date).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn import_note(db: State<DbState>, text: String) -> Result<ImportSummary, String> {
    let conn = db.0.lock().unwrap();
    queries::import_note(&conn, &text).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn export_all_days(db: State<DbState>) -> Result<export::ExportAllSummary, String> {
    let conn = db.0.lock().unwrap();
    export::export_all_days(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn search_tasks(db: State<DbState>, query: String) -> Result<Vec<SearchResult>, String> {
    let conn = db.0.lock().unwrap();
    queries::search_tasks(&conn, &query).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn summarize_range(db: State<DbState>, start: String, end: String) -> Result<RangeSummary, String> {
    let conn = db.0.lock().unwrap();
    queries::summarize_range(&conn, &start, &end).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_backlog(db: State<DbState>) -> Result<Vec<BacklogItem>, String> {
    let conn = db.0.lock().unwrap();
    queries::get_backlog(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn pull_from_backlog(db: State<DbState>, id: i64) -> Result<DayPayload, String> {
    let conn = db.0.lock().unwrap();
    queries::pull_from_backlog(&conn, id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_backlog_snooze(db: State<DbState>, id: i64, snooze_until: Option<String>) -> Result<Vec<BacklogItem>, String> {
    let conn = db.0.lock().unwrap();
    queries::set_backlog_snooze(&conn, id, snooze_until).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn drop_from_backlog(db: State<DbState>, id: i64) -> Result<Vec<BacklogItem>, String> {
    let conn = db.0.lock().unwrap();
    queries::drop_from_backlog(&conn, id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_close_out_candidates(db: State<DbState>) -> Result<Vec<CloseOutCandidate>, String> {
    let conn = db.0.lock().unwrap();
    let date = today(&conn)?;
    queries::get_close_out_candidates(&conn, &date).map_err(|e| e.to_string())
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecisionInput {
    action: String,
    snooze_until: Option<String>,
}

#[tauri::command]
pub fn run_close_out(db: State<DbState>, decisions: HashMap<i64, DecisionInput>) -> Result<CloseOutResult, String> {
    let conn = db.0.lock().unwrap();
    let date = today(&conn)?;
    let parsed: HashMap<i64, Decision> = decisions
        .into_iter()
        .filter_map(|(id, input)| Decision::from_parts(&input.action, input.snooze_until).map(|d| (id, d)))
        .collect();
    queries::run_close_out(&conn, &date, &parsed).map_err(|e| e.to_string())
}
