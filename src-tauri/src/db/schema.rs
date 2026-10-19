pub const SCHEMA: &str = r#"
-- A task is a persistent entity. It exists independent of any day.
CREATE TABLE IF NOT EXISTS task (
    id           INTEGER PRIMARY KEY,
    title        TEXT    NOT NULL,
    parent_id    INTEGER REFERENCES task(id) ON DELETE CASCADE,
    status       TEXT    NOT NULL DEFAULT 'todo',
    status_override INTEGER NOT NULL DEFAULT 0,
    note         TEXT,
    sort_key     TEXT    NOT NULL,
    created_at   TEXT    NOT NULL,
    completed_at TEXT,
    in_backlog   INTEGER NOT NULL DEFAULT 0,
    snooze_until TEXT,
    routine_item_id INTEGER REFERENCES routine_item(id) ON DELETE SET NULL,
    status_since TEXT NOT NULL
);

-- A day is a container. closed_at IS NULL means "live".
CREATE TABLE IF NOT EXISTS day (
    date      TEXT PRIMARY KEY,
    closed_at TEXT
);

-- The join: which tasks were committed to which day, and what they looked like.
CREATE TABLE IF NOT EXISTS day_entry (
    day_date        TEXT    NOT NULL REFERENCES day(date) ON DELETE CASCADE,
    task_id         INTEGER NOT NULL REFERENCES task(id) ON DELETE CASCADE,
    status_snapshot TEXT,
    note_snapshot   TEXT,
    sort_key        TEXT    NOT NULL,
    carried_from    TEXT,
    PRIMARY KEY (day_date, task_id)
);

CREATE TABLE IF NOT EXISTS routine (
    id          INTEGER PRIMARY KEY,
    name        TEXT    NOT NULL,
    sort_key    TEXT    NOT NULL,
    active      INTEGER NOT NULL DEFAULT 1,
    -- 0 = renders before the day's tasks (e.g. a morning routine), 1 = after
    -- (e.g. a wrap-up routine). Per-routine rather than global, since
    -- different routines naturally belong at different ends of the day.
    after_tasks INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS routine_item (
    id         INTEGER PRIMARY KEY,
    routine_id INTEGER NOT NULL REFERENCES routine(id) ON DELETE CASCADE,
    title      TEXT    NOT NULL,
    sort_key   TEXT    NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_day_entry_task ON day_entry(task_id);
CREATE INDEX IF NOT EXISTS idx_task_parent    ON task(parent_id);
CREATE INDEX IF NOT EXISTS idx_task_snooze    ON task(snooze_until) WHERE snooze_until IS NOT NULL;
"#;
