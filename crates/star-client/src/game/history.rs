use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;

#[derive(Debug, Clone, Default)]
pub struct EncounterRecord {
    pub game_name: String,
    pub tag_line: String,
    pub times_seen: i32,
    pub last_seen_at: String,
    pub last_match_kd: Option<f64>,
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
                last_match_kd REAL,
                PRIMARY KEY (puuid)
            );",
        )?;
        ensure_last_match_kd_column(&conn)?;

        Ok(Self { conn })
    }

    pub fn record_encounter(
        &self,
        puuid: &str,
        game_name: &str,
        tag_line: &str,
        update_identity: bool,
        last_match_kd: Option<f64>,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO encounters (puuid, game_name, tag_line, times_seen, last_seen, last_match_kd)
             VALUES (
                ?1,
                CASE WHEN ?4 THEN ?2 ELSE '' END,
                CASE WHEN ?4 THEN ?3 ELSE '' END,
                1,
                datetime('now'),
                ?5
             )
             ON CONFLICT(puuid) DO UPDATE SET
                game_name = CASE WHEN ?4 THEN ?2 ELSE game_name END,
                tag_line = CASE WHEN ?4 THEN ?3 ELSE tag_line END,
                times_seen = times_seen + 1,
                last_seen = datetime('now'),
                last_match_kd = COALESCE(?5, last_match_kd)",
            rusqlite::params![puuid, game_name, tag_line, update_identity, last_match_kd],
        )?;
        Ok(())
    }

    pub fn update_identity(
        &self,
        puuid: &str,
        game_name: &str,
        tag_line: &str,
        last_match_kd: Option<f64>,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO encounters (puuid, game_name, tag_line, times_seen, last_seen, last_match_kd)
             VALUES (?1, ?2, ?3, 1, datetime('now'), ?4)
             ON CONFLICT(puuid) DO UPDATE SET
                game_name = ?2,
                tag_line = ?3,
                last_match_kd = COALESCE(?4, last_match_kd)",
            rusqlite::params![puuid, game_name, tag_line, last_match_kd],
        )?;
        Ok(())
    }

    pub fn encounter(&self, puuid: &str) -> Option<EncounterRecord> {
        self.conn
            .query_row(
                "SELECT game_name, tag_line, times_seen, last_seen, last_match_kd
                 FROM encounters
                 WHERE puuid = ?1",
                rusqlite::params![puuid],
                |row| {
                    Ok(EncounterRecord {
                        game_name: row.get(0)?,
                        tag_line: row.get(1)?,
                        times_seen: row.get(2)?,
                        last_seen_at: row.get(3)?,
                        last_match_kd: row.get(4)?,
                    })
                },
            )
            .ok()
    }
}

fn ensure_last_match_kd_column(conn: &Connection) -> Result<()> {
    match conn.execute("ALTER TABLE encounters ADD COLUMN last_match_kd REAL", []) {
        Ok(_) => Ok(()),
        Err(err) if err.to_string().contains("duplicate column name") => Ok(()),
        Err(err) => Err(err.into()),
    }
}
