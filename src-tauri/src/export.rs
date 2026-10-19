use std::fmt::Write as _;
use std::path::PathBuf;

use rusqlite::{Connection, Result};

use crate::db::queries::{self, DayPayload, TaskNode};

fn status_label(status: &str) -> &str {
    match status {
        "todo" => "not started",
        "in_review" => "in review",
        other => other,
    }
}

fn render_tasks(tasks: &[TaskNode], depth: usize, out: &mut String) {
    let indent = "    ".repeat(depth);
    for t in tasks {
        match t.status.as_str() {
            "done" => {
                let _ = writeln!(out, "{indent}- [x] {}", t.title);
            }
            "dropped" => {
                let _ = writeln!(out, "{indent}- ~~{}~~ `dropped`", t.title);
            }
            other => {
                let rollup = if !t.children.is_empty() {
                    let done = t.children.iter().filter(|c| c.status == "done").count();
                    format!("  ·  {done}/{}", t.children.len())
                } else {
                    String::new()
                };
                let _ = writeln!(out, "{indent}- {} `{}`{rollup}", t.title, status_label(other));
            }
        }
        if let Some(note) = &t.note {
            let _ = writeln!(out, "{indent}  > {note}");
        }
        render_tasks(&t.children, depth + 1, out);
    }
}

/// Renders a day as a readable markdown mirror, close to the original Apple
/// Note format so it pastes cleanly into docs. This is derived data, never
/// the source of truth - regenerating it from the database is always safe.
pub fn render_day_markdown(day: &DayPayload) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# {}\n", day.date);

    for section in &day.routines {
        let _ = writeln!(out, "## {}", section.name);
        for t in &section.tasks {
            if t.status == "done" {
                let _ = writeln!(out, "- [x] {}", t.title);
            } else {
                let _ = writeln!(out, "- {}", t.title);
            }
        }
        let _ = writeln!(out);
    }

    if !day.tasks.is_empty() {
        let _ = writeln!(out, "## Tasks");
        render_tasks(&day.tasks, 0, &mut out);
    }

    out
}

fn export_dir() -> PathBuf {
    let home = std::env::var("HOME").expect("HOME not set");
    PathBuf::from(home).join("Documents").join("Cadence")
}

fn write_file(date: &str, content: &str) -> std::io::Result<PathBuf> {
    let dir = export_dir();
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{date}.md"));
    std::fs::write(&path, content)?;
    Ok(path)
}

/// Exports one day. Failure here must never roll back or block whatever
/// database change triggered it - the markdown file is purely derived and
/// can always be regenerated, so callers should swallow errors from this
/// (e.g. via `.ok()`) rather than propagate them into a close-out.
pub fn export_day(conn: &Connection, date: &str) -> std::result::Result<PathBuf, Box<dyn std::error::Error>> {
    let day = queries::get_day(conn, date)?;
    let content = render_day_markdown(&day);
    Ok(write_file(date, &content)?)
}

#[derive(serde::Serialize)]
pub struct ExportAllSummary {
    pub count: i64,
    pub directory: String,
}

pub fn export_all_days(conn: &Connection) -> Result<ExportAllSummary> {
    let dates: Vec<String> = conn
        .prepare("SELECT date FROM day ORDER BY date")?
        .query_map([], |r| r.get(0))?
        .collect::<Result<Vec<_>>>()?;

    let mut count = 0i64;
    for date in &dates {
        if export_day(conn, date).is_ok() {
            count += 1;
        }
    }

    Ok(ExportAllSummary { count, directory: export_dir().to_string_lossy().to_string() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::queries::{CarriedBanner, RoutineSection, RoutineTaskNode};

    fn task(title: &str, status: &str, children: Vec<TaskNode>) -> TaskNode {
        TaskNode {
            id: 0,
            parent_id: None,
            title: title.to_string(),
            status: status.to_string(),
            status_override: false,
            note: None,
            children,
        }
    }

    #[test]
    fn renders_done_items_as_checked_boxes() {
        let day = DayPayload {
            date: "2026-09-22".to_string(),
            routines: vec![],
            tasks: vec![task("Calendar", "done", vec![])],
            carried_banner: None::<CarriedBanner>,
        };
        let md = render_day_markdown(&day);
        assert!(md.contains("- [x] Calendar"));
    }

    #[test]
    fn renders_status_tag_and_rollup_for_parent_with_children() {
        let day = DayPayload {
            date: "2026-09-22".to_string(),
            routines: vec![],
            tasks: vec![task(
                "Giraffe flag",
                "wip",
                vec![task("Research it", "done", vec![]), task("Create giraffe flag", "in_review", vec![])],
            )],
            carried_banner: None,
        };
        let md = render_day_markdown(&day);
        assert!(md.contains("Giraffe flag `wip`  ·  1/2"));
        assert!(md.contains("    - [x] Research it"));
        assert!(md.contains("    - Create giraffe flag `in review`"));
    }

    #[test]
    fn renders_dropped_as_strikethrough() {
        let day = DayPayload {
            date: "2026-09-22".to_string(),
            routines: vec![],
            tasks: vec![task("Abandoned idea", "dropped", vec![])],
            carried_banner: None,
        };
        let md = render_day_markdown(&day);
        assert!(md.contains("~~Abandoned idea~~ `dropped`"));
    }

    #[test]
    fn renders_not_started_tag_for_todo_status() {
        let day = DayPayload {
            date: "2026-09-22".to_string(),
            routines: vec![],
            tasks: vec![task("Fresh task", "todo", vec![])],
            carried_banner: None,
        };
        let md = render_day_markdown(&day);
        assert!(md.contains("Fresh task `not started`"));
    }

    #[test]
    fn renders_routine_sections_as_their_own_heading() {
        let day = DayPayload {
            date: "2026-09-22".to_string(),
            routines: vec![RoutineSection {
                routine_id: 1,
                name: "Morning".to_string(),
                after_tasks: false,
                tasks: vec![RoutineTaskNode { id: 1, title: "Hide Phone".to_string(), status: "done".to_string() }],
            }],
            tasks: vec![],
            carried_banner: None,
        };
        let md = render_day_markdown(&day);
        assert!(md.contains("## Morning"));
        assert!(md.contains("- [x] Hide Phone"));
    }
}
