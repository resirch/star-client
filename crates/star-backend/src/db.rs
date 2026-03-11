use anyhow::Result;
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};

pub async fn init_pool(database_url: &str) -> Result<SqlitePool> {
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS users (
            puuid TEXT PRIMARY KEY,
            session_token TEXT NOT NULL UNIQUE,
            client_version TEXT NOT NULL DEFAULT '',
            last_heartbeat TEXT NOT NULL DEFAULT (datetime('now')),
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        )",
    )
    .execute(&pool)
    .await?;

    Ok(pool)
}

pub async fn upsert_user(
    pool: &SqlitePool,
    puuid: &str,
    session_token: &str,
    client_version: &str,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO users (puuid, session_token, client_version, last_heartbeat)
         VALUES ($1, $2, $3, datetime('now'))
         ON CONFLICT(puuid) DO UPDATE SET
            session_token = $2,
            client_version = $3,
            last_heartbeat = datetime('now')",
    )
    .bind(puuid)
    .bind(session_token)
    .bind(client_version)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn update_heartbeat(pool: &SqlitePool, session_token: &str) -> Result<bool> {
    let result =
        sqlx::query("UPDATE users SET last_heartbeat = datetime('now') WHERE session_token = $1")
            .bind(session_token)
            .execute(pool)
            .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn query_star_users(pool: &SqlitePool, puuids: &[String]) -> Result<Vec<String>> {
    if puuids.is_empty() {
        return Ok(Vec::new());
    }

    // Build a query with placeholders
    let placeholders: Vec<String> = (1..=puuids.len()).map(|i| format!("${}", i)).collect();
    let query_str = format!(
        "SELECT puuid FROM users WHERE puuid IN ({}) AND last_heartbeat > datetime('now', '-7 days')",
        placeholders.join(", ")
    );

    let mut query = sqlx::query_scalar::<_, String>(&query_str);
    for puuid in puuids {
        query = query.bind(puuid);
    }

    let results = query.fetch_all(pool).await?;
    Ok(results)
}

pub async fn count_active_users(pool: &SqlitePool) -> Result<i64> {
    let count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM users WHERE last_heartbeat > datetime('now', '-5 minutes')",
    )
    .fetch_one(pool)
    .await?;
    Ok(count.0)
}

pub async fn cleanup_stale(pool: &SqlitePool) -> Result<u64> {
    let result =
        sqlx::query("DELETE FROM users WHERE last_heartbeat < datetime('now', '-30 days')")
            .execute(pool)
            .await?;
    Ok(result.rows_affected())
}
