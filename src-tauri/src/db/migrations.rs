use rusqlite::{Connection, Result};

use super::schema::SCHEMA;

pub fn init(conn: &Connection) -> Result<()> {
    conn.execute_batch(SCHEMA)?;
    // `CREATE TABLE IF NOT EXISTS` above is a no-op for a database that
    // already has the `routine` table from before this column existed, so
    // an existing install needs it added by hand, once.
    add_column_if_missing(conn, "routine", "after_tasks", "INTEGER NOT NULL DEFAULT 0")?;
    Ok(())
}

fn add_column_if_missing(conn: &Connection, table: &str, column: &str, decl: &str) -> Result<()> {
    let exists: bool = conn.prepare(&format!("SELECT 1 FROM pragma_table_info('{table}') WHERE name = ?1"))?.exists(
        rusqlite::params![column],
    )?;
    if !exists {
        conn.execute(&format!("ALTER TABLE {table} ADD COLUMN {column} {decl}"), [])?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn running_init_twice_does_not_fail_on_an_already_migrated_column() {
        let conn = Connection::open_in_memory().unwrap();
        init(&conn).unwrap();
        // Simulates reopening an existing database on a later run - must not
        // error trying to add a column that's already there.
        init(&conn).unwrap();
    }

    #[test]
    fn adds_after_tasks_to_a_pre_existing_routine_table() {
        let conn = Connection::open_in_memory().unwrap();
        // An install from before `after_tasks` existed: create the table by
        // hand without that column, then run today's migration over it.
        conn.execute_batch(
            "CREATE TABLE routine (id INTEGER PRIMARY KEY, name TEXT NOT NULL, sort_key TEXT NOT NULL, active INTEGER NOT NULL DEFAULT 1);
             INSERT INTO routine (id, name, sort_key) VALUES (1, 'Morning', 'm');",
        )
        .unwrap();

        init(&conn).unwrap();

        let after_tasks: i64 =
            conn.query_row("SELECT after_tasks FROM routine WHERE id = 1", [], |r| r.get(0)).unwrap();
        assert_eq!(after_tasks, 0);
    }
}
