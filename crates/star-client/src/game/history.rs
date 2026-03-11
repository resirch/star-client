use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;

#[derive(Debug, Clone, Default)]
pub struct EncounterRecord {
    pub game_name: String,
    pub tag_line: String,
    pub times_seen: i32,
    pub last_seen_at: String,
}

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

    pub fn record_encounter(
        &self,
        puuid: &str,
        game_name: &str,
        tag_line: &str,
        update_identity: bool,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO encounters (puuid, game_name, tag_line, times_seen, last_seen)
             VALUES (
                ?1,
                CASE WHEN ?4 THEN ?2 ELSE '' END,
                CASE WHEN ?4 THEN ?3 ELSE '' END,
                1,
                datetime('now')
             )
             ON CONFLICT(puuid) DO UPDATE SET
                game_name = CASE WHEN ?4 THEN ?2 ELSE game_name END,
                tag_line = CASE WHEN ?4 THEN ?3 ELSE tag_line END,
                times_seen = times_seen + 1,
                last_seen = datetime('now')",
            rusqlite::params![puuid, game_name, tag_line, update_identity],
        )?;
        Ok(())
    }

    pub fn update_identity(&self, puuid: &str, game_name: &str, tag_line: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO encounters (puuid, game_name, tag_line, times_seen, last_seen)
             VALUES (?1, ?2, ?3, 1, datetime('now'))
             ON CONFLICT(puuid) DO UPDATE SET
                game_name = ?2,
                tag_line = ?3",
            rusqlite::params![puuid, game_name, tag_line],
        )?;
        Ok(())
    }

    pub fn encounter(&self, puuid: &str) -> Option<EncounterRecord> {
        self.conn
            .query_row(
                "SELECT game_name, tag_line, times_seen, last_seen
                 FROM encounters
                 WHERE puuid = ?1",
                rusqlite::params![puuid],
                |row| {
                    Ok(EncounterRecord {
                        game_name: row.get(0)?,
                        tag_line: row.get(1)?,
                        times_seen: row.get(2)?,
                        last_seen_at: row.get(3)?,
                    })
                },
            )
            .ok()
    }
}
