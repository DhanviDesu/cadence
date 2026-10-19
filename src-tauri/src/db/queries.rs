use std::collections::HashMap;

use chrono::{Duration, Local, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension, Result};
use serde::Serialize;

use crate::domain::sortkey::key_between;
use crate::domain::status::{derive, Status};
use crate::parse::parse_inline_status;

pub fn today_str() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

#[derive(Serialize, Clone)]
pub struct TaskNode {
    pub id: i64,
    pub parent_id: Option<i64>,
    pub title: String,
    pub status: String,
    pub status_override: bool,
    pub note: Option<String>,
    pub children: Vec<TaskNode>,
}

#[derive(Serialize, Clone)]
pub struct RoutineTaskNode {
    pub id: i64,
    pub title: String,
    pub status: String,
}

#[derive(Serialize, Clone)]
pub struct RoutineSection {
    pub routine_id: i64,
    pub name: String,
    pub after_tasks: bool,
    pub tasks: Vec<RoutineTaskNode>,
}

#[derive(Serialize, Clone)]
pub struct CarriedBanner {
    pub count: i64,
    pub from_date: String,
}

#[derive(Serialize)]
pub struct DayPayload {
    pub date: String,
    pub routines: Vec<RoutineSection>,
    pub tasks: Vec<TaskNode>,
    pub carried_banner: Option<CarriedBanner>,
}

struct Row {
    id: i64,
    parent_id: Option<i64>,
    title: String,
    status: String,
    status_override: bool,
    note: Option<String>,
    sort_key: String,
}

/// Returns the app's actual current day: whichever day is still open, or a
/// freshly-created real-today if none is (first run). This is NOT simply
/// "today by the calendar" - the moment you close a day out, the app's live
/// day becomes tomorrow relative to the real calendar, and every command
/// needs to keep operating on that, not silently fall back to the wall-clock
/// date (which would mean writing into an already-closed, frozen day).
pub fn ensure_today(conn: &Connection) -> Result<String> {
    let open: Option<String> = conn
        .query_row("SELECT date FROM day WHERE closed_at IS NULL ORDER BY date DESC LIMIT 1", [], |r| r.get(0))
        .optional()?;
    match open {
        Some(d) => Ok(d),
        None => {
            let d = today_str();
            conn.execute("INSERT INTO day (date) VALUES (?1)", params![d])?;
            Ok(d)
        }
    }
}

/// Regular (non-routine) tasks only - routine instances live in their own
/// key space (see `sibling_keys`) so they never interleave with normal
/// indent/outdent/reorder logic.
fn fetch_rows(conn: &Connection, date: &str) -> Result<Vec<Row>> {
    // A closed day's status_snapshot is frozen at close-out time; an open
    // day's is always NULL, so this falls through to the live status with no
    // behavior change there. Without the COALESCE, browsing a past day would
    // show whatever a carried-over task's status has drifted to *since*,
    // instead of what was actually true when that day closed.
    let mut stmt = conn.prepare(
        "SELECT t.id, t.parent_id, t.title, COALESCE(e.status_snapshot, t.status), t.status_override, t.note, e.sort_key
           FROM day_entry e JOIN task t ON t.id = e.task_id
          WHERE e.day_date = ?1 AND t.routine_item_id IS NULL
          ORDER BY e.sort_key",
    )?;
    let rows = stmt
        .query_map(params![date], |r| {
            Ok(Row {
                id: r.get(0)?,
                parent_id: r.get(1)?,
                title: r.get(2)?,
                status: r.get(3)?,
                status_override: r.get::<_, i64>(4)? != 0,
                note: r.get(5)?,
                sort_key: r.get(6)?,
            })
        })?
        .collect::<Result<Vec<_>>>()?;
    Ok(rows)
}

fn fetch_routine_sections(conn: &Connection, date: &str) -> Result<Vec<RoutineSection>> {
    let mut stmt = conn.prepare(
        "SELECT r.id, r.name, r.after_tasks, t.id, t.title, t.status
           FROM routine r
           JOIN routine_item ri ON ri.routine_id = r.id
           JOIN task t ON t.routine_item_id = ri.id
           JOIN day_entry e ON e.task_id = t.id AND e.day_date = ?1
          WHERE r.active = 1
          ORDER BY r.sort_key, ri.sort_key",
    )?;
    let rows: Vec<(i64, String, bool, i64, String, String)> = stmt
        .query_map(params![date], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get::<_, i64>(2)? != 0, r.get(3)?, r.get(4)?, r.get(5)?))
        })?
        .collect::<Result<Vec<_>>>()?;

    let mut sections: Vec<RoutineSection> = Vec::new();
    for (routine_id, name, after_tasks, task_id, title, status) in rows {
        match sections.last_mut() {
            Some(s) if s.routine_id == routine_id => {
                s.tasks.push(RoutineTaskNode { id: task_id, title, status });
            }
            _ => sections.push(RoutineSection {
                routine_id,
                name,
                after_tasks,
                tasks: vec![RoutineTaskNode { id: task_id, title, status }],
            }),
        }
    }
    Ok(sections)
}

fn build_tree(rows: Vec<Row>) -> Vec<TaskNode> {
    let mut children_of: HashMap<Option<i64>, Vec<Row>> = HashMap::new();
    let present_ids: std::collections::HashSet<i64> = rows.iter().map(|r| r.id).collect();
    for row in rows {
        let parent = row.parent_id.filter(|p| present_ids.contains(p));
        children_of.entry(parent).or_default().push(row);
    }

    fn build(
        parent: Option<i64>,
        children_of: &mut std::collections::HashMap<Option<i64>, Vec<Row>>,
    ) -> Vec<TaskNode> {
        let Some(mut rows) = children_of.remove(&parent) else {
            return Vec::new();
        };
        rows.sort_by(|a, b| a.sort_key.cmp(&b.sort_key));
        rows.into_iter()
            .map(|r| {
                let children = build(Some(r.id), children_of);
                let display_status = if !children.is_empty() && !r.status_override {
                    let child_statuses: Vec<Status> = children
                        .iter()
                        .filter_map(|c| Status::from_str(&c.status))
                        .collect();
                    derive(&child_statuses).as_str().to_string()
                } else {
                    r.status.clone()
                };
                TaskNode {
                    id: r.id,
                    parent_id: r.parent_id,
                    title: r.title,
                    status: display_status,
                    status_override: r.status_override,
                    note: r.note,
                    children,
                }
            })
            .collect()
    }

    build(None, &mut children_of)
}

pub fn get_day(conn: &Connection, date: &str) -> Result<DayPayload> {
    get_day_with_banner(conn, date, None)
}

fn get_day_with_banner(conn: &Connection, date: &str, carried_banner: Option<CarriedBanner>) -> Result<DayPayload> {
    let rows = fetch_rows(conn, date)?;
    Ok(DayPayload {
        date: date.to_string(),
        routines: fetch_routine_sections(conn, date)?,
        tasks: build_tree(rows),
        carried_banner,
    })
}

/// Sort keys are only unique within a sibling group (every new group starts
/// fresh near the middle of the alphabet), so callers that need to find a
/// specific sibling must go through its task id, never look it up by key alone.
fn sibling_keys(conn: &Connection, date: &str, parent_id: Option<i64>) -> Result<Vec<(String, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT e.sort_key, t.id FROM day_entry e JOIN task t ON t.id = e.task_id
          WHERE e.day_date = ?1 AND t.parent_id IS ?2 AND t.routine_item_id IS NULL
          ORDER BY e.sort_key",
    )?;
    let keys = stmt
        .query_map(params![date, parent_id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?
        .collect::<Result<Vec<_>>>()?;
    Ok(keys)
}

fn key_of(conn: &Connection, date: &str, task_id: i64) -> Result<String> {
    conn.query_row(
        "SELECT sort_key FROM day_entry WHERE day_date = ?1 AND task_id = ?2",
        params![date, task_id],
        |r| r.get(0),
    )
}

/// Creates a new sibling task after `after_id` (or at the end if None) under `parent_id`.
pub fn create_task(
    conn: &Connection,
    date: &str,
    parent_id: Option<i64>,
    after_id: Option<i64>,
    title: &str,
) -> Result<DayPayload> {
    let parsed = parse_inline_status(title);
    let siblings = sibling_keys(conn, date, parent_id)?;

    // None means "append at the end" (of this parent's sibling group), not
    // "insert before everything" - matters once other siblings already exist.
    let after_key = match after_id {
        Some(id) => Some(key_of(conn, date, id)?),
        None => siblings.last().map(|(k, _)| k.clone()),
    };
    let next_key = match &after_key {
        Some(k) => siblings.iter().find(|(s, _)| s.as_str() > k.as_str()).map(|(k, _)| k.clone()),
        None => siblings.first().map(|(k, _)| k.clone()),
    };
    let new_key = key_between(after_key.as_deref(), next_key.as_deref());

    let now = chrono::Local::now().to_rfc3339();
    let status = parsed.status.unwrap_or(Status::Todo);
    let task_sort_key = key_between(None, None);

    conn.execute(
        "INSERT INTO task (title, parent_id, status, note, sort_key, created_at, status_since)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
        params![parsed.title, parent_id, status.as_str(), parsed.note, task_sort_key, now],
    )?;
    let task_id = conn.last_insert_rowid();

    conn.execute(
        "INSERT INTO day_entry (day_date, task_id, sort_key) VALUES (?1, ?2, ?3)",
        params![date, task_id, new_key],
    )?;

    get_day(conn, date)
}

pub fn update_task_title(conn: &Connection, date: &str, task_id: i64, title: &str) -> Result<DayPayload> {
    let parsed = parse_inline_status(title);
    if let Some(status) = parsed.status {
        let now = chrono::Local::now().to_rfc3339();
        conn.execute(
            "UPDATE task SET title = ?1, note = ?2, status = ?3, status_since = ?4 WHERE id = ?5",
            params![parsed.title, parsed.note, status.as_str(), now, task_id],
        )?;
    } else {
        conn.execute(
            "UPDATE task SET title = ?1 WHERE id = ?2",
            params![parsed.title, task_id],
        )?;
    }
    get_day(conn, date)
}

pub fn set_status(conn: &Connection, date: &str, task_id: i64, status: &str) -> Result<DayPayload> {
    let now = chrono::Local::now().to_rfc3339();
    let has_children: i64 = conn.query_row(
        "SELECT COUNT(*) FROM task WHERE parent_id = ?1",
        params![task_id],
        |r| r.get(0),
    )?;
    let override_flag = if has_children > 0 { 1 } else { 0 };
    conn.execute(
        "UPDATE task SET status = ?1, status_since = ?2, status_override = ?3 WHERE id = ?4",
        params![status, now, override_flag, task_id],
    )?;
    get_day(conn, date)
}

pub fn clear_status_override(conn: &Connection, date: &str, task_id: i64) -> Result<DayPayload> {
    conn.execute(
        "UPDATE task SET status_override = 0 WHERE id = ?1",
        params![task_id],
    )?;
    get_day(conn, date)
}

pub fn toggle_done(conn: &Connection, date: &str, task_id: i64) -> Result<DayPayload> {
    let current: String = conn.query_row(
        "SELECT status FROM task WHERE id = ?1",
        params![task_id],
        |r| r.get(0),
    )?;
    let next = if current == "done" { Status::Todo } else { Status::Done };
    set_status(conn, date, task_id, next.as_str())
}

pub fn delete_task(conn: &Connection, date: &str, task_id: i64) -> Result<DayPayload> {
    conn.execute("DELETE FROM task WHERE id = ?1", params![task_id])?;
    get_day(conn, date)
}

/// Moves a task to become a child of its previous sibling (no-op if it has none).
pub fn indent_task(conn: &Connection, date: &str, task_id: i64) -> Result<DayPayload> {
    let parent_id: Option<i64> = conn.query_row(
        "SELECT parent_id FROM task WHERE id = ?1",
        params![task_id],
        |r| r.get(0),
    )?;
    let my_key = key_of(conn, date, task_id)?;
    let siblings = sibling_keys(conn, date, parent_id)?;
    let Some((_, new_parent_id)) = siblings.iter().filter(|(k, _)| k.as_str() < my_key.as_str()).max_by(|a, b| a.0.cmp(&b.0)) else {
        return get_day(conn, date);
    };
    let new_parent_id = *new_parent_id;

    let new_siblings = sibling_keys(conn, date, Some(new_parent_id))?;
    let new_key = key_between(new_siblings.last().map(|(k, _)| k.as_str()), None);

    conn.execute("UPDATE task SET parent_id = ?1 WHERE id = ?2", params![new_parent_id, task_id])?;
    conn.execute(
        "UPDATE day_entry SET sort_key = ?1 WHERE day_date = ?2 AND task_id = ?3",
        params![new_key, date, task_id],
    )?;
    get_day(conn, date)
}

/// Moves a task up to become a sibling of its current parent, placed right after it.
pub fn outdent_task(conn: &Connection, date: &str, task_id: i64) -> Result<DayPayload> {
    let parent_id: Option<i64> = conn.query_row(
        "SELECT parent_id FROM task WHERE id = ?1",
        params![task_id],
        |r| r.get(0),
    )?;
    let Some(parent_id) = parent_id else {
        return get_day(conn, date);
    };
    let grandparent_id: Option<i64> = conn.query_row(
        "SELECT parent_id FROM task WHERE id = ?1",
        params![parent_id],
        |r| r.get(0),
    )?;
    let parent_key = key_of(conn, date, parent_id)?;
    let gp_siblings = sibling_keys(conn, date, grandparent_id)?;
    let next_key = gp_siblings
        .iter()
        .find(|(k, _)| k.as_str() > parent_key.as_str())
        .map(|(k, _)| k.clone());
    let new_key = key_between(Some(&parent_key), next_key.as_deref());

    conn.execute("UPDATE task SET parent_id = ?1 WHERE id = ?2", params![grandparent_id, task_id])?;
    conn.execute(
        "UPDATE day_entry SET sort_key = ?1 WHERE day_date = ?2 AND task_id = ?3",
        params![new_key, date, task_id],
    )?;
    get_day(conn, date)
}

pub fn move_task(conn: &Connection, date: &str, task_id: i64, direction: &str) -> Result<DayPayload> {
    let parent_id: Option<i64> = conn.query_row(
        "SELECT parent_id FROM task WHERE id = ?1",
        params![task_id],
        |r| r.get(0),
    )?;
    let my_key = key_of(conn, date, task_id)?;
    let siblings = sibling_keys(conn, date, parent_id)?;

    let new_key = if direction == "up" {
        let Some((prev, _)) = siblings.iter().filter(|(k, _)| k.as_str() < my_key.as_str()).max_by(|a, b| a.0.cmp(&b.0)) else {
            return get_day(conn, date);
        };
        let before_prev = siblings
            .iter()
            .filter(|(k, _)| k.as_str() < prev.as_str())
            .max_by(|a, b| a.0.cmp(&b.0))
            .map(|(k, _)| k.as_str());
        key_between(before_prev, Some(prev))
    } else {
        let Some((next, _)) = siblings.iter().filter(|(k, _)| k.as_str() > my_key.as_str()).min_by(|a, b| a.0.cmp(&b.0)) else {
            return get_day(conn, date);
        };
        let after_next = siblings
            .iter()
            .filter(|(k, _)| k.as_str() > next.as_str())
            .min_by(|a, b| a.0.cmp(&b.0))
            .map(|(k, _)| k.as_str());
        key_between(Some(next), after_next)
    };

    conn.execute(
        "UPDATE day_entry SET sort_key = ?1 WHERE day_date = ?2 AND task_id = ?3",
        params![new_key, date, task_id],
    )?;
    get_day(conn, date)
}

// ---------------------------------------------------------------------------
// Routines
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct RoutineItemDef {
    pub id: i64,
    pub title: String,
}

#[derive(Serialize)]
pub struct RoutineDef {
    pub id: i64,
    pub name: String,
    pub after_tasks: bool,
    pub items: Vec<RoutineItemDef>,
}

pub fn list_routine_defs(conn: &Connection) -> Result<Vec<RoutineDef>> {
    let mut routines: Vec<RoutineDef> = conn
        .prepare("SELECT id, name, after_tasks FROM routine WHERE active = 1 ORDER BY sort_key")?
        .query_map([], |r| {
            Ok(RoutineDef {
                id: r.get(0)?,
                name: r.get(1)?,
                after_tasks: r.get::<_, i64>(2)? != 0,
                items: Vec::new(),
            })
        })?
        .collect::<Result<Vec<_>>>()?;

    for routine in &mut routines {
        routine.items = conn
            .prepare("SELECT id, title FROM routine_item WHERE routine_id = ?1 ORDER BY sort_key")?
            .query_map(params![routine.id], |r| Ok(RoutineItemDef { id: r.get(0)?, title: r.get(1)? }))?
            .collect::<Result<Vec<_>>>()?;
    }
    Ok(routines)
}

fn last_routine_sort_key(conn: &Connection) -> Result<Option<String>> {
    conn.query_row("SELECT sort_key FROM routine ORDER BY sort_key DESC LIMIT 1", [], |r| r.get(0)).optional()
}

pub fn create_routine(conn: &Connection, name: &str) -> Result<Vec<RoutineDef>> {
    let sort_key = key_between(last_routine_sort_key(conn)?.as_deref(), None);
    conn.execute("INSERT INTO routine (name, sort_key) VALUES (?1, ?2)", params![name, sort_key])?;
    list_routine_defs(conn)
}

pub fn add_routine_item(conn: &Connection, routine_id: i64, title: &str) -> Result<Vec<RoutineDef>> {
    let last_key: Option<String> = conn
        .query_row(
            "SELECT sort_key FROM routine_item WHERE routine_id = ?1 ORDER BY sort_key DESC LIMIT 1",
            params![routine_id],
            |r| r.get(0),
        )
        .optional()?;
    let sort_key = key_between(last_key.as_deref(), None);
    conn.execute(
        "INSERT INTO routine_item (routine_id, title, sort_key) VALUES (?1, ?2, ?3)",
        params![routine_id, title, sort_key],
    )?;
    // Backfill today too, so a routine item added mid-day shows up immediately
    // rather than waiting for tomorrow's close-out to mint it.
    let today = ensure_today(conn)?;
    mint_routine_tasks(conn, &today)?;
    list_routine_defs(conn)
}

/// Deletes a routine and its items (cascades via the schema's FK). Task
/// instances already minted from those items are NOT deleted - they lose
/// their routine_item_id (set null by the schema's FK) and simply become
/// ordinary tasks, so today's history isn't silently wiped by deleting a
/// template.
pub fn delete_routine(conn: &Connection, routine_id: i64) -> Result<Vec<RoutineDef>> {
    conn.execute("DELETE FROM routine WHERE id = ?1", params![routine_id])?;
    list_routine_defs(conn)
}

/// Renames a routine item's template. Already-minted task instances keep
/// their own (already-copied) title - only future mints see the new name.
pub fn update_routine_item(conn: &Connection, item_id: i64, title: &str) -> Result<Vec<RoutineDef>> {
    conn.execute("UPDATE routine_item SET title = ?1 WHERE id = ?2", params![title, item_id])?;
    list_routine_defs(conn)
}

/// Deletes a routine item template. Already-minted task instances survive
/// and become ordinary tasks (routine_item_id set null by the schema's FK),
/// same detachment behavior as deleting the whole routine.
pub fn delete_routine_item(conn: &Connection, item_id: i64) -> Result<Vec<RoutineDef>> {
    conn.execute("DELETE FROM routine_item WHERE id = ?1", params![item_id])?;
    list_routine_defs(conn)
}

/// Same swap-with-a-neighbor logic `move_task` uses, generalized over a
/// plain (sort_key, id) sibling list instead of a day-scoped query.
fn swapped_key(siblings: &[(String, i64)], my_key: &str, direction: &str) -> Option<String> {
    if direction == "up" {
        let (prev, _) = siblings.iter().filter(|(k, _)| k.as_str() < my_key).max_by(|a, b| a.0.cmp(&b.0))?;
        let before_prev = siblings
            .iter()
            .filter(|(k, _)| k.as_str() < prev.as_str())
            .max_by(|a, b| a.0.cmp(&b.0))
            .map(|(k, _)| k.as_str());
        Some(key_between(before_prev, Some(prev)))
    } else {
        let (next, _) = siblings.iter().filter(|(k, _)| k.as_str() > my_key).min_by(|a, b| a.0.cmp(&b.0))?;
        let after_next = siblings
            .iter()
            .filter(|(k, _)| k.as_str() > next.as_str())
            .min_by(|a, b| a.0.cmp(&b.0))
            .map(|(k, _)| k.as_str());
        Some(key_between(Some(next), after_next))
    }
}

pub fn move_routine(conn: &Connection, routine_id: i64, direction: &str) -> Result<Vec<RoutineDef>> {
    let my_key: String = conn.query_row("SELECT sort_key FROM routine WHERE id = ?1", params![routine_id], |r| r.get(0))?;
    let siblings: Vec<(String, i64)> = conn
        .prepare("SELECT sort_key, id FROM routine ORDER BY sort_key")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<Vec<_>>>()?;
    if let Some(new_key) = swapped_key(&siblings, &my_key, direction) {
        conn.execute("UPDATE routine SET sort_key = ?1 WHERE id = ?2", params![new_key, routine_id])?;
    }
    list_routine_defs(conn)
}

pub fn set_routine_after_tasks(conn: &Connection, routine_id: i64, after_tasks: bool) -> Result<Vec<RoutineDef>> {
    conn.execute(
        "UPDATE routine SET after_tasks = ?1 WHERE id = ?2",
        params![after_tasks as i64, routine_id],
    )?;
    list_routine_defs(conn)
}

pub fn move_routine_item(conn: &Connection, item_id: i64, direction: &str) -> Result<Vec<RoutineDef>> {
    let (my_key, routine_id): (String, i64) = conn.query_row(
        "SELECT sort_key, routine_id FROM routine_item WHERE id = ?1",
        params![item_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let siblings: Vec<(String, i64)> = conn
        .prepare("SELECT sort_key, id FROM routine_item WHERE routine_id = ?1 ORDER BY sort_key")?
        .query_map(params![routine_id], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<Vec<_>>>()?;
    if let Some(new_key) = swapped_key(&siblings, &my_key, direction) {
        conn.execute("UPDATE routine_item SET sort_key = ?1 WHERE id = ?2", params![new_key, item_id])?;
    }
    list_routine_defs(conn)
}

/// Creates today's/tomorrow's task instances from each active routine's
/// template items. Idempotent - safe to call more than once for the same date.
fn mint_routine_tasks(conn: &Connection, date: &str) -> Result<()> {
    let items: Vec<(i64, String, String)> = conn
        .prepare(
            "SELECT ri.id, ri.title, ri.sort_key FROM routine_item ri
               JOIN routine r ON r.id = ri.routine_id
              WHERE r.active = 1
              ORDER BY r.sort_key, ri.sort_key",
        )?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<Result<Vec<_>>>()?;

    for (item_id, title, sort_key) in items {
        let already: i64 = conn.query_row(
            "SELECT COUNT(*) FROM task t JOIN day_entry e ON e.task_id = t.id
              WHERE t.routine_item_id = ?1 AND e.day_date = ?2",
            params![item_id, date],
            |r| r.get(0),
        )?;
        if already > 0 {
            continue;
        }
        let now = chrono::Local::now().to_rfc3339();
        conn.execute(
            "INSERT INTO task (title, status, sort_key, created_at, status_since, routine_item_id)
             VALUES (?1, 'todo', ?2, ?3, ?3, ?4)",
            params![title, sort_key, now, item_id],
        )?;
        let task_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO day_entry (day_date, task_id, sort_key) VALUES (?1, ?2, ?3)",
            params![date, task_id, sort_key],
        )?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Close-out / rollover
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decision {
    CarryOver,
    Drop,
    Done,
    /// Parked out of the daily list entirely; optionally resurfaces on the
    /// given YYYY-MM-DD date via `surface_due_snoozes`.
    Backlog(Option<String>),
}

impl Decision {
    pub fn from_parts(action: &str, snooze_until: Option<String>) -> Option<Decision> {
        match action {
            "carry_over" => Some(Decision::CarryOver),
            "drop" => Some(Decision::Drop),
            "done" => Some(Decision::Done),
            "backlog" => Some(Decision::Backlog(snooze_until)),
            _ => None,
        }
    }
}

#[derive(Serialize)]
pub struct CloseOutCandidate {
    pub id: i64,
    pub title: String,
    pub breadcrumb: Vec<String>,
    pub status: String,
    pub stale: bool,
}

pub struct CloseOutSummary {
    pub carried_count: i64,
    pub dropped_count: i64,
    pub done_count: i64,
    pub backlogged_count: i64,
    pub tomorrow: String,
}

fn next_day(date: &str) -> String {
    let d = NaiveDate::parse_from_str(date, "%Y-%m-%d").expect("invalid date");
    (d + Duration::days(1)).format("%Y-%m-%d").to_string()
}

/// Every non-routine task today, needing a close-out decision - including
/// ones already done or dropped (with the task's status instead of the
/// unused sort_key), so either can still be revisited and overridden here.
fn eligible_for_close_out(conn: &Connection, date: &str) -> Result<Vec<(i64, String)>> {
    let mut stmt = conn.prepare(
        "SELECT t.id, t.status FROM day_entry e JOIN task t ON t.id = e.task_id
          WHERE e.day_date = ?1 AND t.routine_item_id IS NULL",
    )?;
    let rows = stmt
        .query_map(params![date], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<Vec<_>>>()?;
    Ok(rows)
}

struct TreeRow {
    id: i64,
    parent_id: Option<i64>,
    title: String,
    status: String,
    status_since: String,
    sort_key: String,
}

/// Candidates in the same depth-first order the outliner renders them in
/// Every task is a candidate, including ones already done or dropped, so
/// either can be revisited and overridden here - with breadcrumbs built for
/// free from the walk instead of a query per task.
pub fn get_close_out_candidates(conn: &Connection, date: &str) -> Result<Vec<CloseOutCandidate>> {
    let rows: Vec<TreeRow> = conn
        .prepare(
            "SELECT t.id, t.parent_id, t.title, t.status, t.status_since, e.sort_key
               FROM day_entry e JOIN task t ON t.id = e.task_id
              WHERE e.day_date = ?1 AND t.routine_item_id IS NULL",
        )?
        .query_map(params![date], |r| {
            Ok(TreeRow {
                id: r.get(0)?,
                parent_id: r.get(1)?,
                title: r.get(2)?,
                status: r.get(3)?,
                status_since: r.get(4)?,
                sort_key: r.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>>>()?;

    let by_id: HashMap<i64, &TreeRow> = rows.iter().map(|r| (r.id, r)).collect();

    // A task's parent might not be present in today's list at all (it fell
    // off a previous carry-over) - treat it as top-level rather than losing
    // the whole orphaned subtree, same fallback the regular Day View uses.
    let mut children_of: HashMap<Option<i64>, Vec<&TreeRow>> = HashMap::new();
    for r in &rows {
        let effective_parent = r.parent_id.filter(|p| by_id.contains_key(p));
        children_of.entry(effective_parent).or_default().push(r);
    }
    for kids in children_of.values_mut() {
        kids.sort_by(|a, b| a.sort_key.cmp(&b.sort_key));
    }

    fn walk(
        parent: Option<i64>,
        children_of: &HashMap<Option<i64>, Vec<&TreeRow>>,
        by_id: &HashMap<i64, &TreeRow>,
        out: &mut Vec<CloseOutCandidate>,
    ) {
        let Some(kids) = children_of.get(&parent) else { return };
        for row in kids {
            let mut breadcrumb = Vec::new();
            let mut cur = row.parent_id;
            while let Some(pid) = cur {
                let Some(p) = by_id.get(&pid) else { break };
                breadcrumb.push(p.title.clone());
                cur = p.parent_id;
            }
            let stale = matches!(row.status.as_str(), "wip" | "in_review" | "submitting")
                && crate::domain::staleness::is_stale(&row.status_since);
            out.push(CloseOutCandidate {
                id: row.id,
                title: row.title.clone(),
                breadcrumb,
                status: row.status.clone(),
                stale,
            });
            walk(Some(row.id), children_of, by_id, out);
        }
    }

    let mut out = Vec::new();
    walk(None, &children_of, &by_id, &mut out);
    Ok(out)
}

/// Pulls a single backlog task onto `target`, under whatever parent it last
/// had (falling back to top-level if that parent isn't present that day -
/// same graceful fallback the regular tree view relies on elsewhere).
fn surface_task_onto_day(conn: &Connection, target: &str, task_id: i64, parent_id: Option<i64>) -> Result<()> {
    let siblings = sibling_keys(conn, target, parent_id)?;
    let new_key = key_between(siblings.last().map(|(k, _)| k.as_str()), None);
    conn.execute("UPDATE task SET in_backlog = 0, snooze_until = NULL WHERE id = ?1", params![task_id])?;
    conn.execute(
        "INSERT OR IGNORE INTO day_entry (day_date, task_id, sort_key) VALUES (?1, ?2, ?3)",
        params![target, task_id, new_key],
    )?;
    Ok(())
}

/// Resurfaces every backlog item whose snooze date has arrived.
fn surface_due_snoozes(conn: &Connection, target: &str) -> Result<i64> {
    let due: Vec<(i64, Option<i64>)> = conn
        .prepare("SELECT id, parent_id FROM task WHERE in_backlog = 1 AND snooze_until IS NOT NULL AND snooze_until <= ?1")?
        .query_map(params![target], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<Vec<_>>>()?;

    let count = due.len() as i64;
    for (task_id, parent_id) in due {
        surface_task_onto_day(conn, target, task_id, parent_id)?;
    }
    Ok(count)
}

// ---------------------------------------------------------------------------
// Backlog
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct BacklogItem {
    pub id: i64,
    pub title: String,
    pub status: String,
    pub snooze_until: Option<String>,
    pub breadcrumb: Vec<String>,
}

fn ancestor_breadcrumb(conn: &Connection, parent_id: Option<i64>) -> Result<Vec<String>> {
    let mut breadcrumb = Vec::new();
    let mut cur = parent_id;
    while let Some(pid) = cur {
        let found: Option<(String, Option<i64>)> = conn
            .query_row("SELECT title, parent_id FROM task WHERE id = ?1", params![pid], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .optional()?;
        match found {
            Some((title, next_parent)) => {
                breadcrumb.push(title);
                cur = next_parent;
            }
            None => break,
        }
    }
    Ok(breadcrumb)
}

/// Dated items soonest-first, undated ones (parked indefinitely) last.
pub fn get_backlog(conn: &Connection) -> Result<Vec<BacklogItem>> {
    let rows: Vec<(i64, String, String, Option<String>, Option<i64>)> = conn
        .prepare(
            "SELECT id, title, status, snooze_until, parent_id FROM task
              WHERE in_backlog = 1
              ORDER BY snooze_until IS NULL, snooze_until, title",
        )?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?
        .collect::<Result<Vec<_>>>()?;

    let mut items = Vec::with_capacity(rows.len());
    for (id, title, status, snooze_until, parent_id) in rows {
        items.push(BacklogItem {
            id,
            title,
            status,
            snooze_until,
            breadcrumb: ancestor_breadcrumb(conn, parent_id)?,
        });
    }
    Ok(items)
}

/// Pulls a backlog item onto today, regardless of any snooze date it had.
pub fn pull_from_backlog(conn: &Connection, task_id: i64) -> Result<DayPayload> {
    let today = ensure_today(conn)?;
    let parent_id: Option<i64> =
        conn.query_row("SELECT parent_id FROM task WHERE id = ?1", params![task_id], |r| r.get(0))?;
    surface_task_onto_day(conn, &today, task_id, parent_id)?;
    get_day(conn, &today)
}

/// Sets or clears a backlog item's resurface date. `None` parks it
/// indefinitely again.
pub fn set_backlog_snooze(conn: &Connection, task_id: i64, snooze_until: Option<String>) -> Result<Vec<BacklogItem>> {
    conn.execute("UPDATE task SET snooze_until = ?1 WHERE id = ?2", params![snooze_until, task_id])?;
    get_backlog(conn)
}

/// Abandons a backlog item outright rather than waiting for it to resurface.
pub fn drop_from_backlog(conn: &Connection, task_id: i64) -> Result<Vec<BacklogItem>> {
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "UPDATE task SET status = 'dropped', status_since = ?1, in_backlog = 0, snooze_until = NULL WHERE id = ?2",
        params![now, task_id],
    )?;
    get_backlog(conn)
}

/// Closes `date` and opens `target` (normally the very next day, but the
/// unattended weekend-gap rollover jumps straight to today instead of
/// materializing a "day" - with routine tasks minted and nobody there to do
/// them - for every date in between). Every eligible task gets a decision -
/// explicit, or CarryOver by default for that unattended case. Carrying
/// reuses the task's own day_entry sort_key so relative order survives the
/// jump untouched.
///
/// Carrying a task always carries its whole ancestor chain too (as long as
/// that ancestor was actually present in `date`'s list) - a subtask can't be
/// nested under a parent that has no day_entry tomorrow, and structurally
/// forcing the parent along beats silently promoting the child to top-level.
/// This overrides whatever decision (or lack of one) that ancestor had.
fn close_out_day_into(
    conn: &Connection,
    date: &str,
    target: &str,
    decisions: &HashMap<i64, Decision>,
) -> Result<CloseOutSummary> {
    conn.execute(
        "UPDATE day_entry
            SET status_snapshot = (SELECT status FROM task WHERE task.id = day_entry.task_id),
                note_snapshot   = (SELECT note   FROM task WHERE task.id = day_entry.task_id)
          WHERE day_date = ?1",
        params![date],
    )?;

    conn.execute("INSERT OR IGNORE INTO day (date) VALUES (?1)", params![target])?;
    mint_routine_tasks(conn, target)?;

    let candidates = eligible_for_close_out(conn, date)?;

    // A task already done or dropped, left untouched, defaults back to its
    // own current status rather than CarryOver, so leaving it alone behaves
    // exactly as it did before either became decidable here - it simply
    // doesn't carry forward unless the wizard explicitly says to.
    let default_decision = |status: &str| match status {
        "dropped" => Decision::Drop,
        "done" => Decision::Done,
        _ => Decision::CarryOver,
    };

    // Tasks explicitly carried over despite currently being done/dropped -
    // the default decision for those statuses is never CarryOver (see
    // above), so ending up here means the wizard overrode it on purpose.
    // Carrying them forward still showing "done"/"dropped" would make the
    // override invisible, so these get reopened to "todo" below as well.
    let mut reopen_ids: std::collections::HashSet<i64> = std::collections::HashSet::new();

    let mut carry_ids: std::collections::HashSet<i64> = std::collections::HashSet::new();
    for (id, status) in &candidates {
        let decision = decisions.get(id).cloned().unwrap_or_else(|| default_decision(status));
        if decision == Decision::CarryOver {
            carry_ids.insert(*id);
            if status == "done" || status == "dropped" {
                reopen_ids.insert(*id);
            }
        }
    }
    // Close the carry set under "ancestors present today must come along too".
    let mut stack: Vec<i64> = carry_ids.iter().copied().collect();
    while let Some(id) = stack.pop() {
        let parent_id: Option<i64> =
            conn.query_row("SELECT parent_id FROM task WHERE id = ?1", params![id], |r| r.get(0))?;
        if let Some(pid) = parent_id {
            let present_today: bool = conn
                .query_row(
                    "SELECT 1 FROM day_entry WHERE day_date = ?1 AND task_id = ?2",
                    params![date, pid],
                    |_| Ok(()),
                )
                .optional()?
                .is_some();
            if present_today && carry_ids.insert(pid) {
                stack.push(pid);
            }
        }
    }

    let (mut carried, mut dropped, mut done, mut backlogged) = (0i64, 0i64, 0i64, 0i64);
    let now = chrono::Local::now().to_rfc3339();

    for (task_id, status) in &candidates {
        if carry_ids.contains(task_id) {
            continue;
        }
        let decision = decisions.get(task_id).cloned().unwrap_or_else(|| default_decision(status));
        // Skip the no-op default rather than re-running it: for "dropped"
        // that would bump status_since for nothing, and for "done" it would
        // stomp completed_at with "now" - either way it'd inflate the
        // counts with tasks the user never actually decided on this run.
        if (status == "dropped" && decision == Decision::Drop) || (status == "done" && decision == Decision::Done) {
            continue;
        }
        match decision {
            Decision::Drop => {
                conn.execute("UPDATE task SET status = 'dropped', status_since = ?1 WHERE id = ?2", params![now, task_id])?;
                dropped += 1;
            }
            Decision::Done => {
                conn.execute(
                    "UPDATE task SET status = 'done', status_since = ?1, completed_at = ?1 WHERE id = ?2",
                    params![now, task_id],
                )?;
                done += 1;
            }
            Decision::Backlog(snooze_until) => {
                conn.execute(
                    "UPDATE task SET in_backlog = 1, snooze_until = ?1 WHERE id = ?2",
                    params![snooze_until, task_id],
                )?;
                backlogged += 1;
            }
            Decision::CarryOver => unreachable!("CarryOver decisions are always in carry_ids"),
        }
    }

    for id in &carry_ids {
        // Ancestors forced in here may not be in `candidates` (e.g. already
        // marked done earlier), so look up their sort_key directly.
        let sort_key: Option<String> = conn
            .query_row(
                "SELECT sort_key FROM day_entry WHERE day_date = ?1 AND task_id = ?2",
                params![date, id],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(sort_key) = sort_key {
            conn.execute(
                "INSERT OR IGNORE INTO day_entry (day_date, task_id, sort_key, carried_from)
                 VALUES (?1, ?2, ?3, ?4)",
                params![target, id, sort_key, date],
            )?;
            if reopen_ids.contains(id) {
                conn.execute(
                    "UPDATE task SET status = 'todo', status_since = ?1, completed_at = NULL WHERE id = ?2",
                    params![now, id],
                )?;
            }
            carried += 1;
        }
    }

    conn.execute("UPDATE day SET closed_at = ?2 WHERE date = ?1", params![date, now])?;
    let surfaced = surface_due_snoozes(conn, target)?;
    carried += surfaced;

    // Derived data, never the source of truth - a failed write here must
    // never roll back a close-out that already committed.
    let _ = crate::export::export_day(conn, date);

    Ok(CloseOutSummary { carried_count: carried, dropped_count: dropped, done_count: done, backlogged_count: backlogged, tomorrow: target.to_string() })
}

pub fn close_out_day(conn: &Connection, date: &str, decisions: &HashMap<i64, Decision>) -> Result<CloseOutSummary> {
    close_out_day_into(conn, date, &next_day(date), decisions)
}

#[derive(Serialize)]
pub struct CloseOutResult {
    pub day: DayPayload,
    pub carried: i64,
    pub dropped: i64,
    pub done: i64,
    pub backlogged: i64,
}

/// Runs an explicit, user-driven close-out of `date` with the given decisions
/// and returns the newly-opened next day plus a summary for confirmation.
pub fn run_close_out(conn: &Connection, date: &str, decisions: &HashMap<i64, Decision>) -> Result<CloseOutResult> {
    let summary = close_out_day(conn, date, decisions)?;
    Ok(CloseOutResult {
        day: get_day(conn, &summary.tomorrow)?,
        carried: summary.carried_count,
        dropped: summary.dropped_count,
        done: summary.done_count,
        backlogged: summary.backlogged_count,
    })
}

/// Catches the day up to today. If the most recently open day is stale, it's
/// auto-closed with no triage - everything unfinished carries straight
/// through to today in one jump (per the plan: never block you into a
/// close-out you didn't ask for, and never mint routine-task history for
/// days nobody was here to do them). Returns today's date plus a banner if a
/// gap was closed.
pub fn advance_to_today(conn: &Connection) -> Result<(String, Option<CarriedBanner>)> {
    let open_date = ensure_today(conn)?;
    let today = today_str();
    if open_date >= today {
        return Ok((open_date, None));
    }

    let summary = close_out_day_into(conn, &open_date, &today, &HashMap::new())?;
    Ok((today.clone(), Some(CarriedBanner { count: summary.carried_count, from_date: open_date })))
}

pub fn get_today_payload(conn: &Connection) -> Result<DayPayload> {
    let (date, banner) = advance_to_today(conn)?;
    get_day_with_banner(conn, &date, banner)
}

// ---------------------------------------------------------------------------
// Note import
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct ImportSummary {
    pub historical_days: i64,
    pub tasks_created: i64,
    pub tasks_continued: i64,
    pub carried_to_live: i64,
}

/// What's already on this exact day, keyed the same way the importer's
/// continuity-matching is - used to make re-importing the same note a no-op.
fn existing_lookup_for_day(conn: &Connection, date: &str) -> Result<HashMap<(Option<i64>, String), i64>> {
    let mut stmt = conn.prepare(
        "SELECT t.id, t.parent_id, t.title FROM day_entry e JOIN task t ON t.id = e.task_id
          WHERE e.day_date = ?1 AND t.routine_item_id IS NULL",
    )?;
    let rows: Vec<(i64, Option<i64>, String)> = stmt
        .query_map(params![date], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<Result<Vec<_>>>()?;
    Ok(rows.into_iter().map(|(id, parent_id, title)| ((parent_id, title), id)).collect())
}

/// Recursively inserts one parsed level, matching against `prev_lookup`
/// (built from the previous day's import) by (parent_task_id, title) so a
/// repeated line becomes one continuing task across days instead of a
/// duplicate - this is the whole point of importing into the hybrid model
/// instead of leaving the note snapshot-style.
fn import_level(
    conn: &Connection,
    date: &str,
    parent_task_id: Option<i64>,
    items: &[crate::import::ParsedTask],
    prev_lookup: &HashMap<(Option<i64>, String), i64>,
    new_lookup: &mut HashMap<(Option<i64>, String), i64>,
    created: &mut i64,
    continued: &mut i64,
) -> Result<()> {
    let synthetic_now = format!("{date}T12:00:00+00:00");
    let mut last_key: Option<String> = None;

    for item in items {
        let key = (parent_task_id, item.title.clone());
        let task_id = if let Some(&existing_id) = prev_lookup.get(&key) {
            *continued += 1;
            let current_status: String =
                conn.query_row("SELECT status FROM task WHERE id = ?1", params![existing_id], |r| r.get(0))?;
            if current_status == item.status.as_str() {
                // Status unchanged - leave status_since alone so staleness
                // tracking reflects how long it's *actually* been stuck.
                conn.execute(
                    "UPDATE task SET title = ?1, note = ?2 WHERE id = ?3",
                    params![item.title, item.note, existing_id],
                )?;
            } else {
                conn.execute(
                    "UPDATE task SET title = ?1, note = ?2, status = ?3, status_since = ?4 WHERE id = ?5",
                    params![item.title, item.note, item.status.as_str(), synthetic_now, existing_id],
                )?;
            }
            existing_id
        } else {
            *created += 1;
            conn.execute(
                "INSERT INTO task (title, parent_id, status, note, sort_key, created_at, status_since)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
                params![
                    item.title,
                    parent_task_id,
                    item.status.as_str(),
                    item.note,
                    key_between(None, None),
                    synthetic_now
                ],
            )?;
            conn.last_insert_rowid()
        };

        let sort_key = key_between(last_key.as_deref(), None);
        last_key = Some(sort_key.clone());
        conn.execute(
            "INSERT OR IGNORE INTO day_entry (day_date, task_id, sort_key) VALUES (?1, ?2, ?3)",
            params![date, task_id, sort_key],
        )?;

        new_lookup.insert(key, task_id);
        import_level(conn, date, Some(task_id), &item.children, prev_lookup, new_lookup, created, continued)?;
    }
    Ok(())
}

/// Imports a dumped reverse-chronological daily note. Every day block except
/// the most recent becomes closed history with a frozen snapshot, inserted
/// oldest-first so continuity-matching and status_since both flow forward in
/// real chronological order. The most recent block is carried into the
/// actual current live day via the same `close_out_day_into` the real
/// close-out wizard uses - so its unfinished items land additively on
/// whatever's already live instead of a bespoke overwrite-style merge.
pub fn import_note(conn: &Connection, text: &str) -> Result<ImportSummary> {
    let live_date = ensure_today(conn)?;
    let live = NaiveDate::parse_from_str(&live_date, "%Y-%m-%d").expect("invalid date");
    let anchor = live - Duration::days(1);

    let mut parsed_days = crate::import::parse_note(text, anchor);
    parsed_days.reverse(); // oldest first

    let mut created = 0i64;
    let mut continued = 0i64;
    let mut prev_lookup: HashMap<(Option<i64>, String), i64> = HashMap::new();
    let mut last_date: Option<String> = None;
    let num_days = parsed_days.len();

    for (i, parsed_day) in parsed_days.iter().enumerate() {
        let date_str = parsed_day.date.format("%Y-%m-%d").to_string();
        conn.execute("INSERT OR IGNORE INTO day (date) VALUES (?1)", params![date_str])?;

        // Re-importing the same note must be a no-op, not a duplicate: merge
        // in whatever this exact day already has (if any) on top of the
        // continuity chain, so a matching title+parent resolves to the same
        // task either way.
        let mut lookup = prev_lookup.clone();
        lookup.extend(existing_lookup_for_day(conn, &date_str)?);

        let mut new_lookup: HashMap<(Option<i64>, String), i64> = HashMap::new();
        import_level(conn, &date_str, None, &parsed_day.tasks, &lookup, &mut new_lookup, &mut created, &mut continued)?;
        prev_lookup = new_lookup;

        if i + 1 < num_days {
            // Not the final block - freeze + close it now, before a later
            // block's continuity-match can overwrite these tasks' live
            // status further.
            conn.execute(
                "UPDATE day_entry
                    SET status_snapshot = (SELECT status FROM task WHERE task.id = day_entry.task_id),
                        note_snapshot   = (SELECT note   FROM task WHERE task.id = day_entry.task_id)
                  WHERE day_date = ?1",
                params![date_str],
            )?;
            let now = Local::now().to_rfc3339();
            conn.execute("UPDATE day SET closed_at = ?2 WHERE date = ?1", params![date_str, now])?;
            let _ = crate::export::export_day(conn, &date_str);
        }
        last_date = Some(date_str);
    }

    let carried = if let Some(last) = last_date {
        close_out_day_into(conn, &last, &live_date, &HashMap::new())?.carried_count
    } else {
        0
    };

    Ok(ImportSummary {
        historical_days: num_days.saturating_sub(1) as i64,
        tasks_created: created,
        tasks_continued: continued,
        carried_to_live: carried,
    })
}

// ---------------------------------------------------------------------------
// Search
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct SearchResult {
    pub id: i64,
    pub title: String,
    pub status: String,
    pub note: Option<String>,
    pub breadcrumb: Vec<String>,
    /// Every day this task has appeared on, ascending - a task is one
    /// persistent entity, so this can span many days.
    pub dates: Vec<String>,
}

/// Case-insensitive substring search over title and note, across every day
/// (not just the live one) - routine instances are excluded, same as
/// close-out and the backlog, since they're a fixed small set the user
/// already knows rather than something worth searching for.
pub fn search_tasks(conn: &Connection, query: &str) -> Result<Vec<SearchResult>> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }
    let pattern = format!("%{}%", query.replace('%', "%%").replace('_', "__"));

    let rows: Vec<(i64, String, String, Option<String>, Option<i64>)> = conn
        .prepare(
            "SELECT id, title, status, note, parent_id FROM task
              WHERE routine_item_id IS NULL AND (title LIKE ?1 OR note LIKE ?1)",
        )?
        .query_map(params![pattern], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?
        .collect::<Result<Vec<_>>>()?;

    let mut results = Vec::with_capacity(rows.len());
    for (id, title, status, note, parent_id) in rows {
        let dates: Vec<String> = conn
            .prepare("SELECT day_date FROM day_entry WHERE task_id = ?1 ORDER BY day_date")?
            .query_map(params![id], |r| r.get(0))?
            .collect::<Result<Vec<_>>>()?;
        results.push(SearchResult { id, title, status, note, breadcrumb: ancestor_breadcrumb(conn, parent_id)?, dates });
    }

    // Most recently-seen tasks first.
    results.sort_by(|a, b| b.dates.last().cmp(&a.dates.last()));
    Ok(results)
}

// ---------------------------------------------------------------------------
// Range summary
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct RangeSummaryItem {
    pub id: i64,
    pub title: String,
    pub status: String,
    pub breadcrumb: Vec<String>,
}

#[derive(Serialize)]
pub struct RangeSummary {
    pub start: String,
    pub end: String,
    pub completed: Vec<RangeSummaryItem>,
    pub dropped: Vec<RangeSummaryItem>,
    pub created: Vec<RangeSummaryItem>,
    pub still_active: Vec<RangeSummaryItem>,
}

fn fetch_range_items(conn: &Connection, sql: &str, start: &str, end: &str) -> Result<Vec<RangeSummaryItem>> {
    let rows: Vec<(i64, String, String, Option<i64>)> = conn
        .prepare(sql)?
        .query_map(params![start, end], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
        .collect::<Result<Vec<_>>>()?;
    let mut items = Vec::with_capacity(rows.len());
    for (id, title, status, parent_id) in rows {
        items.push(RangeSummaryItem { id, title, status, breadcrumb: ancestor_breadcrumb(conn, parent_id)? });
    }
    Ok(items)
}

/// Summarizes "what did I actually do" between `start` and `end` (inclusive,
/// plain YYYY-MM-DD dates): tasks that became done or dropped in that window
/// (by `status_since`, so a task only counts once - the day its status
/// actually changed, not every day it happened to carry through at that
/// status beforehand), tasks newly created in that window (by `created_at`),
/// and tasks still open that were on the list for at least one day in the
/// window (by `day_entry`, so you can see what was actively worked without
/// having wrapped up yet).
pub fn summarize_range(conn: &Connection, start: &str, end: &str) -> Result<RangeSummary> {
    let completed = fetch_range_items(
        conn,
        "SELECT id, title, status, parent_id FROM task
          WHERE status = 'done' AND substr(status_since, 1, 10) BETWEEN ?1 AND ?2
          ORDER BY status_since",
        start,
        end,
    )?;
    let dropped = fetch_range_items(
        conn,
        "SELECT id, title, status, parent_id FROM task
          WHERE status = 'dropped' AND substr(status_since, 1, 10) BETWEEN ?1 AND ?2
          ORDER BY status_since",
        start,
        end,
    )?;
    let created = fetch_range_items(
        conn,
        "SELECT id, title, status, parent_id FROM task
          WHERE routine_item_id IS NULL AND substr(created_at, 1, 10) BETWEEN ?1 AND ?2
          ORDER BY created_at",
        start,
        end,
    )?;
    let still_active = fetch_range_items(
        conn,
        "SELECT DISTINCT t.id, t.title, t.status, t.parent_id FROM task t
           JOIN day_entry e ON e.task_id = t.id
          WHERE t.status NOT IN ('done', 'dropped') AND t.routine_item_id IS NULL
            AND e.day_date BETWEEN ?1 AND ?2
          ORDER BY t.title",
        start,
        end,
    )?;

    Ok(RangeSummary { start: start.to_string(), end: end.to_string(), completed, dropped, created, still_active })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations;

    fn setup() -> (Connection, String) {
        let conn = Connection::open_in_memory().unwrap();
        migrations::init(&conn).unwrap();
        let date = ensure_today(&conn).unwrap();
        (conn, date)
    }

    fn titles(tasks: &[TaskNode]) -> Vec<&str> {
        tasks.iter().map(|t| t.title.as_str()).collect()
    }

    #[test]
    fn create_task_appears_in_today() {
        let (conn, date) = setup();
        let day = create_task(&conn, &date, None, None, "Hide Phone").unwrap();
        assert_eq!(titles(&day.tasks), vec!["Hide Phone"]);
    }

    #[test]
    fn create_task_parses_inline_status() {
        let (conn, date) = setup();
        let day = create_task(&conn, &date, None, None, "Calendar (done)").unwrap();
        assert_eq!(day.tasks[0].title, "Calendar");
        assert_eq!(day.tasks[0].status, "done");
    }

    #[test]
    fn siblings_stay_ordered_after_multiple_inserts() {
        let (conn, date) = setup();
        create_task(&conn, &date, None, None, "A").unwrap();
        let day = create_task(&conn, &date, None, None, "B").unwrap();
        let first_id = day.tasks[0].id;
        // Insert C between A and B.
        let day = create_task(&conn, &date, None, Some(first_id), "C").unwrap();
        assert_eq!(titles(&day.tasks), vec!["A", "C", "B"]);
    }

    #[test]
    fn indent_makes_task_child_of_previous_sibling() {
        let (conn, date) = setup();
        create_task(&conn, &date, None, None, "Parent").unwrap();
        let day = create_task(&conn, &date, None, None, "Child").unwrap();
        let child_id = day.tasks[1].id;
        let day = indent_task(&conn, &date, child_id).unwrap();
        assert_eq!(day.tasks.len(), 1);
        assert_eq!(titles(&day.tasks[0].children), vec!["Child"]);
    }

    #[test]
    fn outdent_reverses_indent() {
        let (conn, date) = setup();
        create_task(&conn, &date, None, None, "Parent").unwrap();
        let day = create_task(&conn, &date, None, None, "Child").unwrap();
        let child_id = day.tasks[1].id;
        indent_task(&conn, &date, child_id).unwrap();
        let day = outdent_task(&conn, &date, child_id).unwrap();
        assert_eq!(titles(&day.tasks), vec!["Parent", "Child"]);
        assert!(day.tasks[1].children.is_empty());
    }

    #[test]
    fn indent_is_noop_for_first_task() {
        let (conn, date) = setup();
        let day = create_task(&conn, &date, None, None, "Only").unwrap();
        let id = day.tasks[0].id;
        let day = indent_task(&conn, &date, id).unwrap();
        assert_eq!(day.tasks.len(), 1);
        assert!(day.tasks[0].children.is_empty());
    }

    #[test]
    fn parent_status_derives_from_children() {
        let (conn, date) = setup();
        let day = create_task(&conn, &date, None, None, "Parent").unwrap();
        let parent_id = day.tasks[0].id;
        let day = create_task(&conn, &date, Some(parent_id), None, "Child").unwrap();
        let child_id = day.tasks[0].children[0].id;

        set_status(&conn, &date, child_id, "wip").unwrap();
        let day = get_day(&conn, &date).unwrap();
        assert_eq!(day.tasks[0].status, "wip");

        set_status(&conn, &date, child_id, "done").unwrap();
        let day = get_day(&conn, &date).unwrap();
        assert_eq!(day.tasks[0].status, "done");
    }

    #[test]
    fn manual_status_override_pins_parent() {
        let (conn, date) = setup();
        let day = create_task(&conn, &date, None, None, "Parent").unwrap();
        let parent_id = day.tasks[0].id;
        create_task(&conn, &date, Some(parent_id), None, "Child").unwrap();

        // Setting status directly on a task with children pins it (status_override).
        let day = set_status(&conn, &date, parent_id, "blocked").unwrap();
        assert_eq!(day.tasks[0].status, "blocked");
        assert!(day.tasks[0].status_override);

        let day = clear_status_override(&conn, &date, parent_id).unwrap();
        assert!(!day.tasks[0].status_override);
        assert_eq!(day.tasks[0].status, "todo"); // derived: one child, still todo
    }

    #[test]
    fn delete_task_cascades_to_children() {
        let (conn, date) = setup();
        let day = create_task(&conn, &date, None, None, "Parent").unwrap();
        let parent_id = day.tasks[0].id;
        create_task(&conn, &date, Some(parent_id), None, "Child").unwrap();

        let count_before: i64 = conn.query_row("SELECT COUNT(*) FROM task", [], |r| r.get(0)).unwrap();
        assert_eq!(count_before, 2);

        delete_task(&conn, &date, parent_id).unwrap();
        let count_after: i64 = conn.query_row("SELECT COUNT(*) FROM task", [], |r| r.get(0)).unwrap();
        assert_eq!(count_after, 0);
    }

    #[test]
    fn toggle_done_round_trips() {
        let (conn, date) = setup();
        let day = create_task(&conn, &date, None, None, "Task").unwrap();
        let id = day.tasks[0].id;
        let day = toggle_done(&conn, &date, id).unwrap();
        assert_eq!(day.tasks[0].status, "done");
        let day = toggle_done(&conn, &date, id).unwrap();
        assert_eq!(day.tasks[0].status, "todo");
    }

    #[test]
    fn move_task_swaps_order() {
        let (conn, date) = setup();
        create_task(&conn, &date, None, None, "A").unwrap();
        create_task(&conn, &date, None, None, "B").unwrap();
        let day = create_task(&conn, &date, None, None, "C").unwrap();
        let c_id = day.tasks[2].id;

        let day = move_task(&conn, &date, c_id, "up").unwrap();
        assert_eq!(titles(&day.tasks), vec!["A", "C", "B"]);

        let day = move_task(&conn, &date, c_id, "up").unwrap();
        assert_eq!(titles(&day.tasks), vec!["C", "A", "B"]);

        let day = move_task(&conn, &date, c_id, "up").unwrap();
        assert_eq!(titles(&day.tasks), vec!["C", "A", "B"]); // already first, no-op
    }

    /// Every fresh sibling group starts its sort_key numbering near the same
    /// point, so two unrelated groups' first children can collide on the raw
    /// key string. Indenting into one group must never misattribute a task to
    /// the other group just because their keys happen to match.
    #[test]
    fn indent_does_not_misattach_across_colliding_sibling_groups() {
        let (conn, date) = setup();
        let day = create_task(&conn, &date, None, None, "Parent One").unwrap();
        let p1 = day.tasks[0].id;
        let day = create_task(&conn, &date, Some(p1), None, "Child One A").unwrap();
        let c1a = day.tasks[0].children[0].id;

        let day = create_task(&conn, &date, None, None, "Parent Two").unwrap();
        let p2 = day.tasks[1].id;
        create_task(&conn, &date, Some(p2), None, "Child Two A").unwrap();

        // Deepen Parent One's branch: give Child One A a sibling and indent it
        // under Child One A, reaching a third level of nesting.
        let day = create_task(&conn, &date, Some(p1), Some(c1a), "Child One B").unwrap();
        let c1b = day.tasks[0].children[1].id;
        let day = indent_task(&conn, &date, c1b).unwrap();

        assert_eq!(titles(&day.tasks), vec!["Parent One", "Parent Two"]);
        assert_eq!(titles(&day.tasks[0].children), vec!["Child One A"]);
        assert_eq!(titles(&day.tasks[0].children[0].children), vec!["Child One B"]);
        // Parent Two's own child must be untouched by the unrelated indent.
        assert_eq!(titles(&day.tasks[1].children), vec!["Child Two A"]);
    }

    // -- Routines --------------------------------------------------------

    #[test]
    fn routine_item_mints_a_task_for_today() {
        let (conn, date) = setup();
        create_routine(&conn, "Morning").unwrap();
        let routines = list_routine_defs(&conn).unwrap();
        add_routine_item(&conn, routines[0].id, "Hide Phone").unwrap();

        let day = get_day(&conn, &date).unwrap();
        assert_eq!(day.routines.len(), 1);
        assert_eq!(day.routines[0].name, "Morning");
        assert_eq!(day.routines[0].tasks[0].title, "Hide Phone");
        // Regular task tree must stay empty - routines are a separate space.
        assert!(day.tasks.is_empty());
    }

    #[test]
    fn routine_after_tasks_defaults_to_false_and_round_trips() {
        let (conn, date) = setup();
        create_routine(&conn, "Morning").unwrap();
        create_routine(&conn, "Wrap up").unwrap();
        let routines = list_routine_defs(&conn).unwrap();
        let morning_id = routines[0].id;
        let wrap_up_id = routines[1].id;
        assert!(!routines[0].after_tasks);
        assert!(!routines[1].after_tasks);

        add_routine_item(&conn, morning_id, "Hide Phone").unwrap();
        add_routine_item(&conn, wrap_up_id, "Plan tomorrow").unwrap();

        let routines = set_routine_after_tasks(&conn, wrap_up_id, true).unwrap();
        assert!(!routines.iter().find(|r| r.id == morning_id).unwrap().after_tasks);
        assert!(routines.iter().find(|r| r.id == wrap_up_id).unwrap().after_tasks);

        // The day payload's routine sections carry the flag too, so the
        // frontend can split them into "before"/"after" groups without a
        // separate lookup.
        let day = get_day(&conn, &date).unwrap();
        let morning_section = day.routines.iter().find(|r| r.routine_id == morning_id).unwrap();
        let wrap_up_section = day.routines.iter().find(|r| r.routine_id == wrap_up_id).unwrap();
        assert!(!morning_section.after_tasks);
        assert!(wrap_up_section.after_tasks);
    }

    #[test]
    fn routine_tasks_never_collide_with_regular_sibling_ordering() {
        let (conn, date) = setup();
        create_routine(&conn, "Morning").unwrap();
        let routines = list_routine_defs(&conn).unwrap();
        add_routine_item(&conn, routines[0].id, "Hide Phone").unwrap();

        create_task(&conn, &date, None, None, "A").unwrap();
        let day = create_task(&conn, &date, None, None, "B").unwrap();
        assert_eq!(titles(&day.tasks), vec!["A", "B"]);
        assert_eq!(day.routines[0].tasks.len(), 1);
    }

    #[test]
    fn deleting_a_routine_removes_its_items_and_section() {
        let (conn, date) = setup();
        let routines = create_routine(&conn, "Morning").unwrap();
        let routine_id = routines[0].id;
        add_routine_item(&conn, routine_id, "Hide Phone").unwrap();

        let remaining = delete_routine(&conn, routine_id).unwrap();
        assert!(remaining.is_empty());

        let item_count: i64 =
            conn.query_row("SELECT COUNT(*) FROM routine_item", [], |r| r.get(0)).unwrap();
        assert_eq!(item_count, 0);

        let day = get_day(&conn, &date).unwrap();
        assert!(day.routines.is_empty());
    }

    #[test]
    fn deleting_a_routine_turns_its_minted_tasks_into_ordinary_ones() {
        let (conn, date) = setup();
        let routines = create_routine(&conn, "Morning").unwrap();
        let routine_id = routines[0].id;
        add_routine_item(&conn, routine_id, "Hide Phone").unwrap();

        let before = get_day(&conn, &date).unwrap();
        let minted_task_id = before.routines[0].tasks[0].id;

        delete_routine(&conn, routine_id).unwrap();

        // The already-minted task isn't deleted - it survives as a regular
        // task instead of vanishing along with the routine definition.
        let day = get_day(&conn, &date).unwrap();
        assert!(day.routines.is_empty());
        assert_eq!(titles(&day.tasks), vec!["Hide Phone"]);
        assert_eq!(day.tasks[0].id, minted_task_id);
    }

    #[test]
    fn update_routine_item_renames_the_template_only() {
        let (conn, date) = setup();
        let routines = create_routine(&conn, "Morning").unwrap();
        let updated = add_routine_item(&conn, routines[0].id, "Hide Phone").unwrap();
        let item_id = updated[0].items[0].id;

        let day_before = get_day(&conn, &date).unwrap();
        let minted_title_before = day_before.routines[0].tasks[0].title.clone();
        assert_eq!(minted_title_before, "Hide Phone");

        let renamed = update_routine_item(&conn, item_id, "Hide Phone Away").unwrap();
        assert_eq!(renamed[0].items[0].title, "Hide Phone Away");

        // The already-minted instance keeps its original title.
        let day_after = get_day(&conn, &date).unwrap();
        assert_eq!(day_after.routines[0].tasks[0].title, "Hide Phone");
    }

    #[test]
    fn delete_routine_item_detaches_but_does_not_delete_minted_task() {
        let (conn, date) = setup();
        let routines = create_routine(&conn, "Morning").unwrap();
        let updated = add_routine_item(&conn, routines[0].id, "Hide Phone").unwrap();
        add_routine_item(&conn, routines[0].id, "Walk").unwrap();
        let item_id = updated[0].items[0].id;

        let remaining = delete_routine_item(&conn, item_id).unwrap();
        assert_eq!(remaining[0].items.len(), 1);
        assert_eq!(remaining[0].items[0].title, "Walk");

        let day = get_day(&conn, &date).unwrap();
        assert_eq!(day.routines[0].tasks.len(), 1);
        assert_eq!(day.routines[0].tasks[0].title, "Walk");
        assert_eq!(titles(&day.tasks), vec!["Hide Phone"]);
    }

    // -- Historical days -----------------------------------------------------

    #[test]
    fn a_closed_day_shows_its_frozen_snapshot_not_later_drift() {
        let (conn, date) = setup();
        let day = create_task(&conn, &date, None, None, "Task").unwrap();
        let id = day.tasks[0].id;
        set_status(&conn, &date, id, "wip").unwrap();

        // Close out with the task at "wip" - that's what should be frozen.
        run_close_out(&conn, &date, &HashMap::new()).unwrap();

        // The task is still open tomorrow; change its live status further.
        let tomorrow = next_day(&date);
        set_status(&conn, &tomorrow, id, "done").unwrap();

        // Browsing the closed day must still show "wip", not today's "done".
        let historical = get_day(&conn, &date).unwrap();
        assert_eq!(historical.tasks[0].status, "wip");

        let live = get_day(&conn, &tomorrow).unwrap();
        assert_eq!(live.tasks[0].status, "done");
    }

    // -- Backlog -------------------------------------------------------------

    #[test]
    fn backlog_view_lists_dated_items_before_undated_ones() {
        let (conn, date) = setup();
        let day = create_task(&conn, &date, None, None, "Someday").unwrap();
        let someday_id = day.tasks[0].id;
        let day = create_task(&conn, &date, None, None, "Scheduled").unwrap();
        let scheduled_id = day.tasks[1].id;

        let mut decisions = HashMap::new();
        decisions.insert(someday_id, Decision::Backlog(None));
        decisions.insert(scheduled_id, Decision::Backlog(Some(next_day(&next_day(&date)))));
        run_close_out(&conn, &date, &decisions).unwrap();

        let backlog = get_backlog(&conn).unwrap();
        let titles: Vec<&str> = backlog.iter().map(|b| b.title.as_str()).collect();
        assert_eq!(titles, vec!["Scheduled", "Someday"]);
        assert!(backlog[0].snooze_until.is_some());
        assert!(backlog[1].snooze_until.is_none());
    }

    #[test]
    fn pull_from_backlog_surfaces_immediately_on_today() {
        let (conn, date) = setup();
        let day = create_task(&conn, &date, None, None, "Parked").unwrap();
        let id = day.tasks[0].id;
        let mut decisions = HashMap::new();
        decisions.insert(id, Decision::Backlog(None));
        let result = run_close_out(&conn, &date, &decisions).unwrap();
        let live_day = result.day.date;

        assert!(get_backlog(&conn).unwrap().iter().any(|b| b.id == id));

        let day = pull_from_backlog(&conn, id).unwrap();
        assert_eq!(day.date, live_day);
        assert_eq!(titles(&day.tasks), vec!["Parked"]);
        assert!(get_backlog(&conn).unwrap().is_empty());
    }

    #[test]
    fn set_backlog_snooze_updates_or_clears_the_date() {
        let (conn, date) = setup();
        let day = create_task(&conn, &date, None, None, "Parked").unwrap();
        let id = day.tasks[0].id;
        let mut decisions = HashMap::new();
        decisions.insert(id, Decision::Backlog(None));
        run_close_out(&conn, &date, &decisions).unwrap();

        let updated = set_backlog_snooze(&conn, id, Some("2027-01-01".to_string())).unwrap();
        assert_eq!(updated[0].snooze_until.as_deref(), Some("2027-01-01"));

        let cleared = set_backlog_snooze(&conn, id, None).unwrap();
        assert!(cleared[0].snooze_until.is_none());
    }

    #[test]
    fn drop_from_backlog_removes_it_without_surfacing() {
        let (conn, date) = setup();
        let day = create_task(&conn, &date, None, None, "Parked").unwrap();
        let id = day.tasks[0].id;
        let mut decisions = HashMap::new();
        decisions.insert(id, Decision::Backlog(None));
        let result = run_close_out(&conn, &date, &decisions).unwrap();

        let remaining = drop_from_backlog(&conn, id).unwrap();
        assert!(remaining.is_empty());

        let status: String =
            conn.query_row("SELECT status FROM task WHERE id = ?1", params![id], |r| r.get(0)).unwrap();
        assert_eq!(status, "dropped");

        let day = get_day(&conn, &result.day.date).unwrap();
        assert!(day.tasks.is_empty());
    }

    #[test]
    fn backlog_breadcrumb_reflects_ancestor_chain() {
        let (conn, date) = setup();
        let day = create_task(&conn, &date, None, None, "Project").unwrap();
        let parent_id = day.tasks[0].id;
        let day = create_task(&conn, &date, Some(parent_id), None, "Subtask").unwrap();
        let child_id = day.tasks[0].children[0].id;

        let mut decisions = HashMap::new();
        decisions.insert(child_id, Decision::Backlog(None));
        decisions.insert(parent_id, Decision::Drop);
        run_close_out(&conn, &date, &decisions).unwrap();

        let backlog = get_backlog(&conn).unwrap();
        assert_eq!(backlog[0].breadcrumb, vec!["Project".to_string()]);
    }

    // -- Note import ----------------------------------------------------------

    const REAL_SAMPLE_NOTE: &str = r#"
09/22 TODAY
- Hide Phone
- Calendar (done)
- Address test fix CLs (submitting)
- Present my AI automation suite
    - Make AI skills universal (wip)

09/21
- Hide Phone
- Calendar (done)
- Address test fix CLs (in review)
    - NP (in review)
    - SS (in review))
- Present my AI automation suite
    - Make AI skills universal (wip)
- Track Testing scenarios somewhere (done, in ai first doc for now0
"#;

    #[test]
    fn imports_the_real_sample_note_end_to_end() {
        let (conn, _date) = setup();
        let live = today_str();
        let live_date = NaiveDate::parse_from_str(&live, "%Y-%m-%d").unwrap();
        let anchor = live_date - Duration::days(1);
        let parsed = crate::import::parse_note(REAL_SAMPLE_NOTE, anchor);
        let most_recent_date = parsed[0].date.format("%Y-%m-%d").to_string();
        let historical_date = parsed[1].date.format("%Y-%m-%d").to_string();

        let summary = import_note(&conn, REAL_SAMPLE_NOTE).unwrap();
        assert_eq!(summary.historical_days, 1);
        assert!(summary.tasks_created > 0);
        assert!(summary.tasks_continued > 0);

        // The historical day is closed and its snapshot reflects what the
        // note said *that day* - including the doubled-close-paren line.
        let (closed_at, _): (Option<String>, ()) = conn
            .query_row("SELECT closed_at, 1 FROM day WHERE date = ?1", params![historical_date], |r| {
                Ok((r.get(0)?, ()))
            })
            .unwrap();
        assert!(closed_at.is_some());

        let historical = get_day(&conn, &historical_date).unwrap();
        let address_hist = historical.tasks.iter().find(|t| t.title == "Address test fix CLs").unwrap();
        let ss = address_hist.children.iter().find(|t| t.title == "SS").unwrap();
        assert_eq!(ss.status, "in_review"); // doubled close-paren still resolved
        assert!(historical.tasks.iter().any(|t| t.title == "Track Testing scenarios somewhere"));

        // The live day gets the note's unfinished *current* state via continuity,
        // not a disconnected duplicate: "Address test fix CLs" progressed from
        // in_review (09/21) to submitting (09/22) on the SAME task id.
        let live_day = get_day(&conn, &live).unwrap();
        let live_titles = titles(&live_day.tasks);
        assert!(live_titles.contains(&"Hide Phone"));
        assert!(live_titles.contains(&"Address test fix CLs"));
        assert!(live_titles.contains(&"Present my AI automation suite"));
        // Calendar was done in the note - done items don't carry forward.
        assert!(!live_titles.contains(&"Calendar"));
        // Likewise the done-via-typo'd-paren item from history stays behind.
        assert!(!live_titles.contains(&"Track Testing scenarios somewhere"));

        let address_live = live_day.tasks.iter().find(|t| t.title == "Address test fix CLs").unwrap();
        assert_eq!(address_live.status, "submitting");
        assert_eq!(address_live.id, address_hist.id); // same task, continued across days

        let _ = most_recent_date;
    }

    #[test]
    fn reimporting_the_same_note_does_not_duplicate_tasks() {
        let (conn, _date) = setup();
        import_note(&conn, REAL_SAMPLE_NOTE).unwrap();
        let first_count: i64 = conn.query_row("SELECT COUNT(*) FROM task", [], |r| r.get(0)).unwrap();

        // Re-importing the identical note should match everything via
        // continuity rather than creating a second copy of every task.
        import_note(&conn, REAL_SAMPLE_NOTE).unwrap();
        let second_count: i64 = conn.query_row("SELECT COUNT(*) FROM task", [], |r| r.get(0)).unwrap();
        assert_eq!(first_count, second_count);
    }

    // -- Search ---------------------------------------------------------

    #[test]
    fn search_matches_title_case_insensitively() {
        let (conn, date) = setup();
        create_task(&conn, &date, None, None, "Giraffe flag rollout").unwrap();
        create_task(&conn, &date, None, None, "Unrelated task").unwrap();

        let results = search_tasks(&conn, "giraffe").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Giraffe flag rollout");
    }

    #[test]
    fn search_matches_note_content_too() {
        let (conn, date) = setup();
        create_task(&conn, &date, None, None, "Pass giraffe flags into components (maybe later when we bump it)")
            .unwrap();

        let results = search_tasks(&conn, "bump").unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].note.as_deref().unwrap().contains("bump"));
    }

    #[test]
    fn search_lists_every_day_a_task_appeared_on() {
        let (conn, date) = setup();
        let day = create_task(&conn, &date, None, None, "Address test fix CLs").unwrap();
        let id = day.tasks[0].id;
        let result = run_close_out(&conn, &date, &HashMap::new()).unwrap();

        let results = search_tasks(&conn, "address").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, id);
        assert_eq!(results[0].dates, vec![date, result.day.date]);
    }

    #[test]
    fn search_excludes_routine_tasks() {
        let (conn, _date) = setup();
        create_routine(&conn, "Morning").unwrap();
        let routines = list_routine_defs(&conn).unwrap();
        add_routine_item(&conn, routines[0].id, "Hide Phone").unwrap();

        assert!(search_tasks(&conn, "hide").unwrap().is_empty());
    }

    #[test]
    fn search_breadcrumb_reflects_ancestor_chain() {
        let (conn, date) = setup();
        let day = create_task(&conn, &date, None, None, "Project X").unwrap();
        let parent_id = day.tasks[0].id;
        create_task(&conn, &date, Some(parent_id), None, "Findable subtask").unwrap();

        let results = search_tasks(&conn, "findable").unwrap();
        assert_eq!(results[0].breadcrumb, vec!["Project X".to_string()]);
    }

    #[test]
    fn blank_query_returns_nothing() {
        let (conn, date) = setup();
        create_task(&conn, &date, None, None, "Anything").unwrap();
        assert!(search_tasks(&conn, "  ").unwrap().is_empty());
    }

    // -- Close-out ---------------------------------------------------------

    #[test]
    fn close_out_default_carries_everything_over() {
        let (conn, date) = setup();
        create_task(&conn, &date, None, None, "Unfinished").unwrap();
        let day = create_task(&conn, &date, None, None, "Done already").unwrap();
        let done_id = day.tasks[1].id;
        set_status(&conn, &date, done_id, "done").unwrap();

        let result = run_close_out(&conn, &date, &HashMap::new()).unwrap();
        assert_eq!(result.carried, 1); // only the unfinished one
        assert_eq!(result.done, 0);
        assert_eq!(titles(&result.day.tasks), vec!["Unfinished"]);
    }

    #[test]
    fn close_out_respects_explicit_decisions() {
        let (conn, date) = setup();
        let day = create_task(&conn, &date, None, None, "Carry me").unwrap();
        let carry_id = day.tasks[0].id;
        let day = create_task(&conn, &date, None, None, "Drop me").unwrap();
        let drop_id = day.tasks[1].id;
        let day = create_task(&conn, &date, None, None, "Finish me").unwrap();
        let done_id = day.tasks[2].id;

        let mut decisions = HashMap::new();
        decisions.insert(carry_id, Decision::CarryOver);
        decisions.insert(drop_id, Decision::Drop);
        decisions.insert(done_id, Decision::Done);

        let result = run_close_out(&conn, &date, &decisions).unwrap();
        assert_eq!(result.carried, 1);
        assert_eq!(result.dropped, 1);
        assert_eq!(result.done, 1);
        assert_eq!(titles(&result.day.tasks), vec!["Carry me"]);

        let dropped_status: String =
            conn.query_row("SELECT status FROM task WHERE id = ?1", params![drop_id], |r| r.get(0)).unwrap();
        assert_eq!(dropped_status, "dropped");
    }

    #[test]
    fn already_dropped_task_can_still_be_overridden_at_close_out() {
        let (conn, date) = setup();
        let day = create_task(&conn, &date, None, None, "Reconsidered").unwrap();
        let reconsidered_id = day.tasks[0].id;
        set_status(&conn, &date, reconsidered_id, "dropped").unwrap();
        let day = create_task(&conn, &date, None, None, "Still dropped").unwrap();
        let still_dropped_id = day.tasks[1].id;
        set_status(&conn, &date, still_dropped_id, "dropped").unwrap();

        let candidates = get_close_out_candidates(&conn, &date).unwrap();
        assert!(candidates.iter().any(|c| c.id == reconsidered_id));
        assert!(candidates.iter().any(|c| c.id == still_dropped_id));

        let mut decisions = HashMap::new();
        decisions.insert(reconsidered_id, Decision::CarryOver);
        // still_dropped_id gets no explicit decision - leaving an
        // already-dropped task untouched must behave exactly as it did
        // before dropped tasks became decidable: it just stays dropped.

        let result = run_close_out(&conn, &date, &decisions).unwrap();
        assert_eq!(result.carried, 1);
        assert_eq!(result.dropped, 0);
        assert_eq!(titles(&result.day.tasks), vec!["Reconsidered"]);
        // Carrying a dropped task forward on purpose reopens it - otherwise
        // it would show up tomorrow still looking dropped, as if the
        // override had no effect at all.
        assert_eq!(result.day.tasks[0].status.as_str(), "todo");

        let still_dropped_status: String = conn
            .query_row("SELECT status FROM task WHERE id = ?1", params![still_dropped_id], |r| r.get(0))
            .unwrap();
        assert_eq!(still_dropped_status, "dropped");
    }

    #[test]
    fn already_done_task_can_still_be_overridden_at_close_out() {
        let (conn, date) = setup();
        let day = create_task(&conn, &date, None, None, "Actually not done").unwrap();
        let reopened_id = day.tasks[0].id;
        set_status(&conn, &date, reopened_id, "done").unwrap();
        let day = create_task(&conn, &date, None, None, "Really done").unwrap();
        let still_done_id = day.tasks[1].id;
        set_status(&conn, &date, still_done_id, "done").unwrap();

        let candidates = get_close_out_candidates(&conn, &date).unwrap();
        assert!(candidates.iter().any(|c| c.id == reopened_id));
        assert!(candidates.iter().any(|c| c.id == still_done_id));

        let mut decisions = HashMap::new();
        decisions.insert(reopened_id, Decision::CarryOver);
        // still_done_id gets no explicit decision - leaving an already-done
        // task untouched must behave exactly as it did before done tasks
        // became decidable: it just stays done, and doesn't carry forward.

        let result = run_close_out(&conn, &date, &decisions).unwrap();
        assert_eq!(result.carried, 1);
        assert_eq!(result.done, 0);
        assert_eq!(titles(&result.day.tasks), vec!["Actually not done"]);
        // Carrying a done task forward on purpose reopens it - otherwise it
        // would show up tomorrow still looking done, as if the override had
        // no effect at all.
        assert_eq!(result.day.tasks[0].status.as_str(), "todo");

        let still_done_status: String = conn
            .query_row("SELECT status FROM task WHERE id = ?1", params![still_done_id], |r| r.get(0))
            .unwrap();
        assert_eq!(still_done_status, "done");
    }

    #[test]
    fn close_out_freezes_snapshot_and_seals_the_day() {
        let (conn, date) = setup();
        create_task(&conn, &date, None, None, "Task").unwrap();
        run_close_out(&conn, &date, &HashMap::new()).unwrap();

        let (closed_at, snapshot): (Option<String>, Option<String>) = conn
            .query_row(
                "SELECT day.closed_at, day_entry.status_snapshot
                   FROM day JOIN day_entry ON day_entry.day_date = day.date
                  WHERE day.date = ?1",
                params![date],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert!(closed_at.is_some());
        assert_eq!(snapshot.as_deref(), Some("todo"));
    }

    #[test]
    fn close_out_excludes_routine_tasks_from_candidates() {
        let (conn, date) = setup();
        create_routine(&conn, "Morning").unwrap();
        let routines = list_routine_defs(&conn).unwrap();
        add_routine_item(&conn, routines[0].id, "Hide Phone").unwrap();
        create_task(&conn, &date, None, None, "Real task").unwrap();

        let candidates = get_close_out_candidates(&conn, &date).unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].title, "Real task");
    }

    #[test]
    fn stale_task_is_flagged_in_close_out_candidates() {
        let (conn, date) = setup();
        let day = create_task(&conn, &date, None, None, "Old WIP").unwrap();
        let id = day.tasks[0].id;
        set_status(&conn, &date, id, "wip").unwrap();
        let old = (chrono::Local::now() - chrono::Duration::days(5)).to_rfc3339();
        conn.execute("UPDATE task SET status_since = ?1 WHERE id = ?2", params![old, id]).unwrap();

        let candidates = get_close_out_candidates(&conn, &date).unwrap();
        assert!(candidates[0].stale);
    }

    #[test]
    fn breadcrumb_reflects_ancestor_chain() {
        let (conn, date) = setup();
        let day = create_task(&conn, &date, None, None, "Project").unwrap();
        let parent_id = day.tasks[0].id;
        create_task(&conn, &date, Some(parent_id), None, "Subtask").unwrap();

        let candidates = get_close_out_candidates(&conn, &date).unwrap();
        let subtask = candidates.iter().find(|c| c.title == "Subtask").unwrap();
        assert_eq!(subtask.breadcrumb, vec!["Project".to_string()]);
    }

    // -- Weekend-gap auto rollover ------------------------------------------

    #[test]
    fn advance_to_today_is_a_noop_when_already_current() {
        let (conn, date) = setup();
        let (current, banner) = advance_to_today(&conn).unwrap();
        assert_eq!(current, date);
        assert!(banner.is_none());
    }

    #[test]
    fn ensure_today_tracks_the_open_day_not_the_wall_clock_date() {
        // A real close-out (via run_close_out, not the unattended gap path)
        // pushes the app's live day ahead of the real calendar date, since
        // "tomorrow" is only "today" once the actual calendar catches up.
        // Every command routes through ensure_today, so it must follow that
        // open day - not silently fall back to today_str() and start writing
        // into the now-closed, frozen day.
        let (conn, date) = setup();
        create_task(&conn, &date, None, None, "Task").unwrap();
        let result = run_close_out(&conn, &date, &HashMap::new()).unwrap();
        let live_day = next_day(&date);
        assert_eq!(result.day.date, live_day);
        assert_eq!(ensure_today(&conn).unwrap(), live_day);
    }

    #[test]
    fn advance_to_today_auto_carries_across_a_multi_day_gap() {
        let conn = Connection::open_in_memory().unwrap();
        migrations::init(&conn).unwrap();

        let stale_date = "2020-01-01";
        conn.execute("INSERT INTO day (date) VALUES (?1)", params![stale_date]).unwrap();
        create_task(&conn, stale_date, None, None, "Left open").unwrap();

        let (current, banner) = advance_to_today(&conn).unwrap();
        assert_eq!(current, today_str());
        let banner = banner.expect("gap should produce a banner");
        assert_eq!(banner.from_date, stale_date);
        assert_eq!(banner.count, 1);

        let day = get_day(&conn, &current).unwrap();
        assert_eq!(titles(&day.tasks), vec!["Left open"]);

        // The stale day and every day in between should now be closed.
        let open_days: i64 = conn
            .query_row("SELECT COUNT(*) FROM day WHERE closed_at IS NULL", [], |r| r.get(0))
            .unwrap();
        assert_eq!(open_days, 1);
    }

    // -- Ancestor-forced carry ------------------------------------------

    #[test]
    fn carrying_a_child_forces_its_parent_to_carry_too() {
        let (conn, date) = setup();
        let day = create_task(&conn, &date, None, None, "Project").unwrap();
        let parent_id = day.tasks[0].id;
        let day = create_task(&conn, &date, Some(parent_id), None, "Subtask").unwrap();
        let child_id = day.tasks[0].children[0].id;

        // Parent explicitly dropped, but its child is carried over - the
        // parent must come along anyway, or the child would be orphaned.
        let mut decisions = HashMap::new();
        decisions.insert(parent_id, Decision::Drop);
        decisions.insert(child_id, Decision::CarryOver);

        let result = run_close_out(&conn, &date, &decisions).unwrap();
        assert_eq!(titles(&result.day.tasks), vec!["Project"]);
        assert_eq!(titles(&result.day.tasks[0].children), vec!["Subtask"]);

        // The parent's own "dropped" decision must not have been applied.
        let parent_status: String =
            conn.query_row("SELECT status FROM task WHERE id = ?1", params![parent_id], |r| r.get(0)).unwrap();
        assert_eq!(parent_status, "todo");
    }

    #[test]
    fn carrying_a_grandchild_forces_the_whole_ancestor_chain() {
        let (conn, date) = setup();
        let day = create_task(&conn, &date, None, None, "Grandparent").unwrap();
        let gp = day.tasks[0].id;
        let day = create_task(&conn, &date, Some(gp), None, "Parent").unwrap();
        let parent_id = day.tasks[0].children[0].id;
        let day = create_task(&conn, &date, Some(parent_id), None, "Child").unwrap();
        let child_id = day.tasks[0].children[0].children[0].id;

        let mut decisions = HashMap::new();
        decisions.insert(gp, Decision::Done);
        decisions.insert(parent_id, Decision::Drop);
        decisions.insert(child_id, Decision::CarryOver);

        let result = run_close_out(&conn, &date, &decisions).unwrap();
        assert_eq!(titles(&result.day.tasks), vec!["Grandparent"]);
        assert_eq!(titles(&result.day.tasks[0].children), vec!["Parent"]);
        assert_eq!(titles(&result.day.tasks[0].children[0].children), vec!["Child"]);
    }

    #[test]
    fn dropping_a_whole_branch_with_no_carried_descendant_works_normally() {
        let (conn, date) = setup();
        let day = create_task(&conn, &date, None, None, "Project").unwrap();
        let parent_id = day.tasks[0].id;
        create_task(&conn, &date, Some(parent_id), None, "Subtask").unwrap();

        // No decisions given - default is CarryOver for everything, so both
        // should carry without needing the forcing logic to kick in.
        let result = run_close_out(&conn, &date, &HashMap::new()).unwrap();
        assert_eq!(titles(&result.day.tasks), vec!["Project"]);
        assert_eq!(titles(&result.day.tasks[0].children), vec!["Subtask"]);
    }

    // -- Close-out candidate ordering -------------------------------------

    #[test]
    fn candidates_are_returned_in_tree_order_with_breadcrumbs() {
        let (conn, date) = setup();
        create_task(&conn, &date, None, None, "A").unwrap();
        let day = create_task(&conn, &date, None, None, "B").unwrap();
        let b_id = day.tasks[1].id;
        create_task(&conn, &date, Some(b_id), None, "B1").unwrap();

        let candidates = get_close_out_candidates(&conn, &date).unwrap();
        let titles: Vec<&str> = candidates.iter().map(|c| c.title.as_str()).collect();
        assert_eq!(titles, vec!["A", "B", "B1"]);
        assert_eq!(candidates[2].breadcrumb, vec!["B".to_string()]);
    }

    #[test]
    fn candidates_survive_a_missing_parent_instead_of_vanishing() {
        // A child can end up on a day without its parent (parent already
        // done/dropped, or - historically - an ancestor that fell off a
        // carry-over). The walk must still surface it as a top-level
        // candidate rather than silently returning zero candidates for the
        // whole day just because the true root is unreachable.
        let (conn, date) = setup();
        let day = create_task(&conn, &date, None, None, "Parent").unwrap();
        let parent_id = day.tasks[0].id;
        create_task(&conn, &date, Some(parent_id), None, "Child").unwrap();

        conn.execute(
            "DELETE FROM day_entry WHERE day_date = ?1 AND task_id = ?2",
            params![date, parent_id],
        )
        .unwrap();

        let candidates = get_close_out_candidates(&conn, &date).unwrap();
        let titles: Vec<&str> = candidates.iter().map(|c| c.title.as_str()).collect();
        assert_eq!(titles, vec!["Child"]);
        assert!(candidates[0].breadcrumb.is_empty());
    }

    // -- Backlog / snooze ---------------------------------------------------

    #[test]
    fn backlog_without_date_removes_task_from_the_list_and_does_not_resurface() {
        let (conn, date) = setup();
        let day = create_task(&conn, &date, None, None, "Someday").unwrap();
        let id = day.tasks[0].id;

        let mut decisions = HashMap::new();
        decisions.insert(id, Decision::Backlog(None));
        let result = run_close_out(&conn, &date, &decisions).unwrap();

        assert_eq!(result.backlogged, 1);
        assert!(result.day.tasks.is_empty());
        let (in_backlog, snooze): (i64, Option<String>) = conn
            .query_row("SELECT in_backlog, snooze_until FROM task WHERE id = ?1", params![id], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap();
        assert_eq!(in_backlog, 1);
        assert!(snooze.is_none());
    }

    #[test]
    fn backlog_snoozed_until_the_very_next_day_resurfaces_immediately() {
        let (conn, date) = setup();
        let day = create_task(&conn, &date, None, None, "Snoozed").unwrap();
        let id = day.tasks[0].id;

        let mut decisions = HashMap::new();
        decisions.insert(id, Decision::Backlog(Some(next_day(&date))));
        let result = run_close_out(&conn, &date, &decisions).unwrap();
        // Due date is the very day being opened - surfaces right away rather
        // than making you close out twice, matching the plan's noted edge case.
        assert_eq!(titles(&result.day.tasks), vec!["Snoozed"]);
    }

    #[test]
    fn backlog_with_later_due_date_resurfaces_on_that_future_close_out() {
        let (conn, date) = setup();
        let day = create_task(&conn, &date, None, None, "Snoozed").unwrap();
        let id = day.tasks[0].id;
        let due = next_day(&next_day(&date)); // two days out

        let mut decisions = HashMap::new();
        decisions.insert(id, Decision::Backlog(Some(due)));
        let result = run_close_out(&conn, &date, &decisions).unwrap();
        assert!(result.day.tasks.is_empty()); // not due tomorrow yet

        let tomorrow = next_day(&date);
        let result2 = run_close_out(&conn, &tomorrow, &HashMap::new()).unwrap();
        assert_eq!(titles(&result2.day.tasks), vec!["Snoozed"]);
    }

    #[test]
    fn backlog_with_future_date_does_not_resurface_early() {
        let (conn, date) = setup();
        let day = create_task(&conn, &date, None, None, "Later").unwrap();
        let id = day.tasks[0].id;

        let far_future = next_day(&next_day(&next_day(&date)));
        let mut decisions = HashMap::new();
        decisions.insert(id, Decision::Backlog(Some(far_future)));
        run_close_out(&conn, &date, &decisions).unwrap();

        let tomorrow = next_day(&date);
        let result = run_close_out(&conn, &tomorrow, &HashMap::new()).unwrap();
        assert!(result.day.tasks.is_empty());
    }

    // -- Range summary ---------------------------------------------------

    #[test]
    fn range_summary_buckets_completed_dropped_created_and_active() {
        let (conn, date) = setup();
        let day = create_task(&conn, &date, None, None, "Finish this").unwrap();
        let done_id = day.tasks[0].id;
        let day = create_task(&conn, &date, None, None, "Abandon this").unwrap();
        let dropped_id = day.tasks[1].id;
        create_task(&conn, &date, None, None, "Still going").unwrap();
        set_status(&conn, &date, done_id, "done").unwrap();
        set_status(&conn, &date, dropped_id, "dropped").unwrap();

        let summary = summarize_range(&conn, &date, &date).unwrap();
        assert_eq!(summary.completed.len(), 1);
        assert_eq!(summary.completed[0].title, "Finish this");
        assert_eq!(summary.dropped.len(), 1);
        assert_eq!(summary.dropped[0].title, "Abandon this");
        // All three were created today, regardless of where they ended up.
        assert_eq!(summary.created.len(), 3);
        assert_eq!(summary.still_active.len(), 1);
        assert_eq!(summary.still_active[0].title, "Still going");
    }

    #[test]
    fn range_summary_excludes_events_outside_the_window() {
        let (conn, date) = setup();
        let day = create_task(&conn, &date, None, None, "Old task").unwrap();
        let id = day.tasks[0].id;
        set_status(&conn, &date, id, "done").unwrap();

        let before = NaiveDate::parse_from_str(&date, "%Y-%m-%d").unwrap() - Duration::days(10);
        let before_str = before.format("%Y-%m-%d").to_string();
        let also_before = NaiveDate::parse_from_str(&date, "%Y-%m-%d").unwrap() - Duration::days(5);
        let also_before_str = also_before.format("%Y-%m-%d").to_string();

        let summary = summarize_range(&conn, &before_str, &also_before_str).unwrap();
        assert!(summary.completed.is_empty());
        assert!(summary.created.is_empty());
    }

    #[test]
    fn range_summary_counts_a_status_change_only_once_even_if_carried_before() {
        // A task that sat as WIP for days before finally being marked done
        // should only show up in "completed" for the day it actually finished.
        let (conn, date) = setup();
        let day = create_task(&conn, &date, None, None, "Long runner").unwrap();
        let id = day.tasks[0].id;
        set_status(&conn, &date, id, "wip").unwrap();
        let result = run_close_out(&conn, &date, &HashMap::new()).unwrap();
        let tomorrow = result.day.date;
        set_status(&conn, &tomorrow, id, "done").unwrap();
        // set_status always stamps the real wall-clock moment - in this
        // instantaneous test run that's indistinguishable between `date` and
        // `tomorrow`, so force it to reflect the logical day it changed on,
        // simulating real elapsed time between the two status changes.
        let synthetic = format!("{tomorrow}T09:00:00+00:00");
        conn.execute("UPDATE task SET status_since = ?1 WHERE id = ?2", params![synthetic, id]).unwrap();

        let summary_yesterday = summarize_range(&conn, &date, &date).unwrap();
        assert!(summary_yesterday.completed.is_empty());

        let summary_today = summarize_range(&conn, &tomorrow, &tomorrow).unwrap();
        assert_eq!(summary_today.completed.len(), 1);
    }

    #[test]
    fn range_summary_breadcrumb_reflects_ancestor_chain() {
        let (conn, date) = setup();
        let day = create_task(&conn, &date, None, None, "Project Y").unwrap();
        let parent_id = day.tasks[0].id;
        create_task(&conn, &date, Some(parent_id), None, "Shipped subtask").unwrap();
        let day = get_day(&conn, &date).unwrap();
        let child_id = day.tasks[0].children[0].id;
        set_status(&conn, &date, child_id, "done").unwrap();

        let summary = summarize_range(&conn, &date, &date).unwrap();
        let shipped = summary.completed.iter().find(|i| i.title == "Shipped subtask").unwrap();
        assert_eq!(shipped.breadcrumb, vec!["Project Y".to_string()]);
    }

    // -- Routine / routine item reordering -----------------------------

    #[test]
    fn move_routine_swaps_order() {
        let (conn, _date) = setup();
        create_routine(&conn, "Morning").unwrap();
        create_routine(&conn, "Evening").unwrap();
        let routines = list_routine_defs(&conn).unwrap();
        let (morning, evening) = (routines[0].id, routines[1].id);

        let updated = move_routine(&conn, evening, "up").unwrap();
        assert_eq!(updated.iter().map(|r| r.id).collect::<Vec<_>>(), vec![evening, morning]);
    }

    #[test]
    fn move_routine_up_is_noop_when_already_first() {
        let (conn, _date) = setup();
        create_routine(&conn, "Morning").unwrap();
        create_routine(&conn, "Evening").unwrap();
        let routines = list_routine_defs(&conn).unwrap();
        let morning = routines[0].id;

        let updated = move_routine(&conn, morning, "up").unwrap();
        assert_eq!(updated[0].id, morning);
    }

    #[test]
    fn move_routine_item_swaps_order_within_its_routine() {
        let (conn, _date) = setup();
        let routines = create_routine(&conn, "Morning").unwrap();
        add_routine_item(&conn, routines[0].id, "Hide Phone").unwrap();
        add_routine_item(&conn, routines[0].id, "Calendar").unwrap();
        let routines = list_routine_defs(&conn).unwrap();
        let second_item = routines[0].items[1].id;

        let updated = move_routine_item(&conn, second_item, "up").unwrap();
        let titles: Vec<&str> = updated[0].items.iter().map(|i| i.title.as_str()).collect();
        assert_eq!(titles, vec!["Calendar", "Hide Phone"]);
    }

    #[test]
    fn move_routine_item_does_not_cross_into_another_routine() {
        let (conn, _date) = setup();
        create_routine(&conn, "Morning").unwrap();
        create_routine(&conn, "Evening").unwrap();
        let routines = list_routine_defs(&conn).unwrap();
        add_routine_item(&conn, routines[0].id, "Only morning item").unwrap();
        let routines = list_routine_defs(&conn).unwrap();
        let item_id = routines[0].items[0].id;

        // Nothing to swap with within its own routine - must stay put, not
        // jump into the other routine's items.
        let updated = move_routine_item(&conn, item_id, "down").unwrap();
        let morning = updated.iter().find(|r| r.name == "Morning").unwrap();
        assert_eq!(morning.items.len(), 1);
        assert_eq!(morning.items[0].id, item_id);
    }
}
