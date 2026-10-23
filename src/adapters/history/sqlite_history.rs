use anyhow::{Context, Result};
use rusqlite::Connection;
use crate::config::data_dir;
use crate::domain::ports::history::HistoryStore;

pub struct SqliteHistory {
    conn: Connection,
}

impl SqliteHistory {
    pub fn open() -> Result<Self> {
        let dir = data_dir();
        std::fs::create_dir_all(&dir)?;
        let conn = Connection::open(dir.join("history.db"))
            .context("open history.db")?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS history (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                created_at  TEXT    DEFAULT (datetime('now')),
                raw         TEXT    NOT NULL,
                text        TEXT    NOT NULL,
                duration_ms INTEGER
            );"
        ).context("create history table")?;
        Ok(Self { conn })
    }
}

impl HistoryStore for SqliteHistory {
    fn save(&mut self, raw: &str, text: &str, duration_ms: u64) -> Result<()> {
        self.conn.execute(
            "INSERT INTO history (raw, text, duration_ms) VALUES (?1, ?2, ?3)",
            rusqlite::params![raw, text, duration_ms as i64],
        ).context("insert history")?;
        Ok(())
    }
}

