use anyhow::Result;
use rusqlite::{params, Connection};

use crate::task::Task;

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS tasks (
                id         INTEGER PRIMARY KEY AUTOINCREMENT,
                title      TEXT    NOT NULL,
                position   INTEGER NOT NULL DEFAULT 0,
                done       INTEGER NOT NULL DEFAULT 0,
                created_at TEXT    NOT NULL DEFAULT '',
                notes      TEXT    NOT NULL DEFAULT ''
            );",
        )?;
        // Migrations: add columns that may be missing in older DBs.
        let _ = conn.execute(
            "ALTER TABLE tasks ADD COLUMN created_at TEXT NOT NULL DEFAULT ''",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE tasks ADD COLUMN notes TEXT NOT NULL DEFAULT ''",
            [],
        );
        Ok(Self { conn })
    }

    pub fn load_active(&self) -> Result<Vec<Task>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, position, done, created_at, notes
             FROM tasks WHERE done = 0 ORDER BY position",
        )?;
        Self::rows_to_tasks(&mut stmt)
    }

    pub fn load_done(&self) -> Result<Vec<Task>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, position, done, created_at, notes
             FROM tasks WHERE done = 1 ORDER BY id DESC",
        )?;
        Self::rows_to_tasks(&mut stmt)
    }

    fn rows_to_tasks(stmt: &mut rusqlite::Statement) -> Result<Vec<Task>> {
        let tasks = stmt
            .query_map([], |row| {
                Ok(Task {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    position: row.get(2)?,
                    done: row.get::<_, i32>(3)? != 0,
                    created_at: row.get(4)?,
                    notes: row.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(tasks)
    }

    pub fn insert(&self, title: &str, position: i64, created_at: &str) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO tasks (title, position, done, created_at, notes)
             VALUES (?1, ?2, 0, ?3, '')",
            params![title, position, created_at],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn set_position(&self, id: i64, position: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE tasks SET position = ?1 WHERE id = ?2",
            params![position, id],
        )?;
        Ok(())
    }

    pub fn set_done(&self, id: i64, done: bool) -> Result<()> {
        self.conn.execute(
            "UPDATE tasks SET done = ?1 WHERE id = ?2",
            params![done as i32, id],
        )?;
        Ok(())
    }

    pub fn set_notes(&self, id: i64, notes: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE tasks SET notes = ?1 WHERE id = ?2",
            params![notes, id],
        )?;
        Ok(())
    }

    pub fn delete(&self, id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM tasks WHERE id = ?1", params![id])?;
        Ok(())
    }
}
