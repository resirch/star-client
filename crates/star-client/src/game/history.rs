use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;

#[derive(Debug, Clone, Default)]
pub struct EncounterRecord {
    pub game_name: String,
    pub tag_line: String,
    pub times_seen: i32,
    pub last_seen_at: String,
    pub last_map_name: String,
    pub last_agent_name: String,
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
                last_match_id TEXT NOT NULL DEFAULT '',
                previous_last_seen TEXT,
                map_name TEXT NOT NULL DEFAULT '',
                agent_name TEXT NOT NULL DEFAULT '',
                previous_map_name TEXT,
                previous_agent_name TEXT,
                PRIMARY KEY (puuid)
            );",
        )?;
        ensure_last_match_kd_column(&conn)?;
        ensure_last_match_id_column(&conn)?;
        ensure_previous_last_seen_column(&conn)?;
        ensure_map_name_column(&conn)?;
        ensure_agent_name_column(&conn)?;
        ensure_previous_map_name_column(&conn)?;
        ensure_previous_agent_name_column(&conn)?;

        Ok(Self { conn })
    }

    pub fn record_encounter(
        &self,
        puuid: &str,
        match_id: &str,
        game_name: &str,
        tag_line: &str,
        map_name: &str,
        agent_name: &str,
        update_identity: bool,
        last_match_kd: Option<f64>,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO encounters (
                puuid,
                game_name,
                tag_line,
                times_seen,
                last_seen,
                last_match_kd,
                last_match_id,
                map_name,
                agent_name
             )
             VALUES (
                ?1,
                CASE WHEN ?7 THEN ?3 ELSE '' END,
                CASE WHEN ?7 THEN ?4 ELSE '' END,
                1,
                datetime('now'),
                ?8,
                ?2,
                ?5,
                ?6
             )
             ON CONFLICT(puuid) DO UPDATE SET
                game_name = CASE WHEN ?7 THEN ?3 ELSE game_name END,
                tag_line = CASE WHEN ?7 THEN ?4 ELSE tag_line END,
                times_seen = times_seen + CASE
                    WHEN last_match_id = excluded.last_match_id THEN 0
                    ELSE 1
                END,
                previous_last_seen = CASE
                    WHEN last_match_id = excluded.last_match_id THEN previous_last_seen
                    ELSE last_seen
                END,
                previous_map_name = CASE
                    WHEN last_match_id = excluded.last_match_id THEN previous_map_name
                    ELSE map_name
                END,
                previous_agent_name = CASE
                    WHEN last_match_id = excluded.last_match_id THEN previous_agent_name
                    ELSE agent_name
                END,
                last_seen = CASE
                    WHEN last_match_id = excluded.last_match_id THEN last_seen
                    ELSE datetime('now')
                END,
                map_name = CASE
                    WHEN last_match_id = excluded.last_match_id AND excluded.map_name = ''
                        THEN map_name
                    ELSE excluded.map_name
                END,
                agent_name = CASE
                    WHEN last_match_id = excluded.last_match_id AND excluded.agent_name = ''
                        THEN agent_name
                    ELSE excluded.agent_name
                END,
                last_match_kd = COALESCE(?8, last_match_kd),
                last_match_id = excluded.last_match_id",
            rusqlite::params![
                puuid,
                match_id,
                game_name,
                tag_line,
                map_name,
                agent_name,
                update_identity,
                last_match_kd
            ],
        )?;
        Ok(())
    }

    pub fn update_match_details(
        &self,
        puuid: &str,
        match_id: &str,
        map_name: &str,
        agent_name: &str,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE encounters
             SET map_name = CASE WHEN ?3 = '' THEN map_name ELSE ?3 END,
                 agent_name = CASE WHEN ?4 = '' THEN agent_name ELSE ?4 END
             WHERE puuid = ?1 AND last_match_id = ?2",
            rusqlite::params![puuid, match_id, map_name, agent_name],
        )?;
        Ok(())
    }

    pub fn continue_match(&self, previous_match_id: &str, match_id: &str) -> Result<()> {
        if previous_match_id.is_empty() || match_id.is_empty() || previous_match_id == match_id {
            return Ok(());
        }

        self.conn.execute(
            "UPDATE encounters
             SET last_match_id = ?2
             WHERE last_match_id = ?1",
            rusqlite::params![previous_match_id, match_id],
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

    pub fn encounter(&self, puuid: &str, match_id: &str) -> Option<EncounterRecord> {
        self.conn
            .query_row(
                "SELECT
                    game_name,
                    tag_line,
                    CASE
                        WHEN last_match_id = ?2 THEN MAX(times_seen - 1, 0)
                        ELSE times_seen
                    END,
                    CASE
                        WHEN last_match_id = ?2 THEN COALESCE(previous_last_seen, '')
                        ELSE last_seen
                    END,
                    CASE
                        WHEN last_match_id = ?2 THEN COALESCE(previous_map_name, '')
                        ELSE map_name
                    END,
                    CASE
                        WHEN last_match_id = ?2 THEN COALESCE(previous_agent_name, '')
                        ELSE agent_name
                    END,
                    last_match_kd
                 FROM encounters
                 WHERE puuid = ?1",
                rusqlite::params![puuid, match_id],
                |row| {
                    Ok(EncounterRecord {
                        game_name: row.get(0)?,
                        tag_line: row.get(1)?,
                        times_seen: row.get(2)?,
                        last_seen_at: row.get(3)?,
                        last_map_name: row.get(4)?,
                        last_agent_name: row.get(5)?,
                        last_match_kd: row.get(6)?,
                    })
                },
            )
            .ok()
    }
}

fn ensure_last_match_kd_column(conn: &Connection) -> Result<()> {
    ensure_encounter_column(conn, "ALTER TABLE encounters ADD COLUMN last_match_kd REAL")
}

fn ensure_last_match_id_column(conn: &Connection) -> Result<()> {
    ensure_encounter_column(
        conn,
        "ALTER TABLE encounters ADD COLUMN last_match_id TEXT NOT NULL DEFAULT ''",
    )
}

fn ensure_previous_last_seen_column(conn: &Connection) -> Result<()> {
    ensure_encounter_column(
        conn,
        "ALTER TABLE encounters ADD COLUMN previous_last_seen TEXT",
    )
}

fn ensure_map_name_column(conn: &Connection) -> Result<()> {
    ensure_encounter_column(
        conn,
        "ALTER TABLE encounters ADD COLUMN map_name TEXT NOT NULL DEFAULT ''",
    )
}

fn ensure_agent_name_column(conn: &Connection) -> Result<()> {
    ensure_encounter_column(
        conn,
        "ALTER TABLE encounters ADD COLUMN agent_name TEXT NOT NULL DEFAULT ''",
    )
}

fn ensure_previous_map_name_column(conn: &Connection) -> Result<()> {
    ensure_encounter_column(
        conn,
        "ALTER TABLE encounters ADD COLUMN previous_map_name TEXT",
    )
}

fn ensure_previous_agent_name_column(conn: &Connection) -> Result<()> {
    ensure_encounter_column(
        conn,
        "ALTER TABLE encounters ADD COLUMN previous_agent_name TEXT",
    )
}

fn ensure_encounter_column(conn: &Connection, sql: &str) -> Result<()> {
    match conn.execute(sql, []) {
        Ok(_) => Ok(()),
        Err(err) if err.to_string().contains("duplicate column name") => Ok(()),
        Err(err) => Err(err.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::PlayerHistory;

    #[test]
    fn restarting_during_a_match_does_not_recount_encounters() {
        let data_dir =
            std::env::temp_dir().join(format!("star-client-history-{}", uuid::Uuid::new_v4()));

        {
            let history = PlayerHistory::open(&data_dir).unwrap();
            history
                .record_encounter(
                    "player-1", "match-1", "Player", "TAG", "Ascent", "Jett", true, None,
                )
                .unwrap();
        }

        {
            let history = PlayerHistory::open(&data_dir).unwrap();
            let current_match = history.encounter("player-1", "match-1").unwrap();
            assert_eq!(current_match.times_seen, 0);
            assert!(current_match.last_seen_at.is_empty());

            history
                .record_encounter(
                    "player-1", "match-1", "Player", "TAG", "Ascent", "Jett", true, None,
                )
                .unwrap();

            let after_restart = history.encounter("player-1", "match-1").unwrap();
            assert_eq!(after_restart.times_seen, 0);
            assert!(after_restart.last_seen_at.is_empty());
        }

        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn current_match_uses_the_previous_match_history() {
        let data_dir =
            std::env::temp_dir().join(format!("star-client-history-{}", uuid::Uuid::new_v4()));

        {
            let history = PlayerHistory::open(&data_dir).unwrap();
            history
                .record_encounter(
                    "player-1", "match-1", "Player", "TAG", "Ascent", "Jett", true, None,
                )
                .unwrap();
            let first_match = history.encounter("player-1", "different-match").unwrap();

            history
                .record_encounter(
                    "player-1", "match-2", "Player", "TAG", "Bind", "Sage", true, None,
                )
                .unwrap();
            let second_match = history.encounter("player-1", "match-2").unwrap();

            assert_eq!(second_match.times_seen, 1);
            assert_eq!(second_match.last_seen_at, first_match.last_seen_at);
            assert_eq!(second_match.last_map_name, "Ascent");
            assert_eq!(second_match.last_agent_name, "Jett");
        }

        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn continued_match_id_does_not_create_an_encounter() {
        let data_dir =
            std::env::temp_dir().join(format!("star-client-history-{}", uuid::Uuid::new_v4()));

        {
            let history = PlayerHistory::open(&data_dir).unwrap();
            history
                .record_encounter(
                    "player-1", "pregame", "Player", "TAG", "Ascent", "", true, None,
                )
                .unwrap();
            history
                .update_match_details("player-1", "pregame", "Ascent", "Jett")
                .unwrap();
            history.continue_match("pregame", "ingame").unwrap();
            history
                .record_encounter(
                    "player-1", "ingame", "Player", "TAG", "Ascent", "Jett", true, None,
                )
                .unwrap();

            let encounter = history.encounter("player-1", "ingame").unwrap();
            assert_eq!(encounter.times_seen, 0);
            assert!(encounter.last_seen_at.is_empty());
        }

        let _ = std::fs::remove_dir_all(data_dir);
    }
}
