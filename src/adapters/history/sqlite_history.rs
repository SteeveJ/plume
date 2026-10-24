use anyhow::{Context, Result};
use rusqlite::Connection;
use std::path::Path;
use crate::config::{ensure_data_dir, restrict_file};
use crate::domain::ports::history::HistoryStore;

const MAX_ROWS: i64 = 10_000;

pub struct SqliteHistory {
    conn: Connection,
}

impl SqliteHistory {
    pub fn open() -> Result<Self> {
        Self::open_at(&ensure_data_dir().join("history.db"))
    }

    pub fn open_at(path: &Path) -> Result<Self> {
        let conn = Connection::open(path).context("open history.db")?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS history (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                created_at  TEXT    DEFAULT (datetime('now')),
                raw         TEXT    NOT NULL,
                text        TEXT    NOT NULL,
                duration_ms INTEGER
            );"
        ).context("create history table")?;
        restrict_file(path);
        Ok(Self { conn })
    }

    fn prune(&self, keep: i64) -> Result<()> {
        self.conn.execute("DELETE FROM history WHERE id <= (SELECT MAX(id) FROM history) - ?1", [keep])
            .context("prune history")?;
        Ok(())
    }
}

impl HistoryStore for SqliteHistory {
    fn save(&mut self, raw: &str, text: &str, duration_ms: u64) -> Result<()> {
        self.conn.execute(
            "INSERT INTO history (raw, text, duration_ms) VALUES (?1, ?2, ?3)",
            rusqlite::params![raw, text, duration_ms as i64],
        ).context("insert history")?;
        self.prune(MAX_ROWS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("plume-{name}-{}.db", std::process::id()))
    }

    #[test]
    fn prune_keeps_only_the_latest_rows() {
        let p = temp_db("prune");
        let mut h = SqliteHistory::open_at(&p).unwrap();
        for i in 0..5 { h.save(&format!("raw{i}"), &format!("text{i}"), 100).unwrap(); }
        h.prune(3).unwrap();
        let oldest: String = h.conn.query_row("SELECT MIN(text) FROM history", [], |r| r.get(0)).unwrap();
        let n: i64 = h.conn.query_row("SELECT COUNT(*) FROM history", [], |r| r.get(0)).unwrap();
        let _ = std::fs::remove_file(&p);
        assert_eq!((n, oldest.as_str()), (3, "text2"));
    }

    #[test]
    fn database_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let p = temp_db("perm");
        std::fs::write(&p, "").unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o644)).unwrap();
        let _h = SqliteHistory::open_at(&p).unwrap();
        let mode = std::fs::metadata(&p).unwrap().permissions().mode() & 0o777;
        let _ = std::fs::remove_file(&p);
        assert_eq!(mode, 0o600);
    }
}
