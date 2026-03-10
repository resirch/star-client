use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;

pub struct PlayerHistory {
    conn: Connection,
}

impl PlayerHistory {
    pub fn open(data_dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(data_dir)?;
        let db_path = data_dir.join("history.db");
        let conn = Connection::open(&db_path)?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS encounters (
                puuid TEXT NOT NULL,
                game_name TEXT NOT NULL DEFAULT '',
                tag_line TEXT NOT NULL DEFAULT '',
                times_seen INTEGER NOT NULL DEFAULT 1,
                last_seen TEXT NOT NULL DEFAULT (datetime('now')),
                PRIMARY KEY (puuid)
            );",
        )?;

        Ok(Self { conn })
    }

    pub fn record_encounter(&self, puuid: &str, game_name: &str, tag_line: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO encounters (puuid, game_name, tag_line, times_seen, last_seen)
             VALUES (?1, ?2, ?3, 1, datetime('now'))
             ON CONFLICT(puuid) DO UPDATE SET
                game_name = ?2,
                tag_line = ?3,
                times_seen = times_seen + 1,
                last_seen = datetime('now')",
            rusqlite::params![puuid, game_name, tag_line],
        )?;
        Ok(())
    }

    pub fn times_seen(&self, puuid: &str) -> i32 {
        self.conn
            .query_row(
                "SELECT times_seen FROM encounters WHERE puuid = ?1",
                rusqlite::params![puuid],
                |row| row.get(0),
            )
            .unwrap_or(0)
    }

    pub fn last_seen(&self, puuid: &str) -> Option<String> {
        self.conn
            .query_row(
                "SELECT last_seen FROM encounters WHERE puuid = ?1",
                rusqlite::params![puuid],
                |row| row.get(0),
            )
            .ok()
    }
}
