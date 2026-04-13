use anyhow::Result;
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};

const ACTIVE_USER_WINDOW_SQL: &str = "-5 minutes";

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
        "SELECT puuid FROM users WHERE puuid IN ({}) AND last_heartbeat > datetime('now', '{}')",
        placeholders.join(", "),
        ACTIVE_USER_WINDOW_SQL
    );

    let mut query = sqlx::query_scalar::<_, String>(&query_str);
    for puuid in puuids {
        query = query.bind(puuid);
    }

    let results = query.fetch_all(pool).await?;
    Ok(results)
}

pub async fn count_active_users(pool: &SqlitePool) -> Result<i64> {
    let query_str = format!(
        "SELECT COUNT(*) FROM users WHERE last_heartbeat > datetime('now', '{}')",
        ACTIVE_USER_WINDOW_SQL
    );
    let count: (i64,) = sqlx::query_as(&query_str).fetch_one(pool).await?;
    Ok(count.0)
}

pub async fn deregister(pool: &SqlitePool, session_token: &str) -> Result<bool> {
    let result = sqlx::query("DELETE FROM users WHERE session_token = $1")
        .bind(session_token)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn cleanup_stale(pool: &SqlitePool) -> Result<u64> {
    let result =
        sqlx::query("DELETE FROM users WHERE last_heartbeat < datetime('now', '-30 days')")
            .execute(pool)
            .await?;
    Ok(result.rows_affected())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_pool() -> SqlitePool {
        let db_path =
            std::env::temp_dir().join(format!("star-backend-test-{}.db", uuid::Uuid::new_v4()));
        let database_url = format!(
            "sqlite:{}?mode=rwc",
            db_path.to_string_lossy().replace('\\', "/")
        );
        init_pool(&database_url).await.expect("create test pool")
    }

    #[tokio::test]
    async fn deregister_removes_user_immediately() {
        let pool = test_pool().await;

        upsert_user(&pool, "player-puuid", "player-token", "1.0.0")
            .await
            .expect("insert user");

        // User should be active
        let star_users = query_star_users(&pool, &["player-puuid".to_string()])
            .await
            .expect("query");
        assert_eq!(star_users, vec!["player-puuid".to_string()]);

        // Deregister
        let removed = deregister(&pool, "player-token").await.expect("deregister");
        assert!(removed);

        // User should no longer appear
        let star_users = query_star_users(&pool, &["player-puuid".to_string()])
            .await
            .expect("query after deregister");
        assert!(star_users.is_empty());
    }

    #[tokio::test]
    async fn query_star_users_only_returns_recent_heartbeats() {
        let pool = test_pool().await;

        upsert_user(&pool, "active-puuid", "active-token", "1.0.0")
            .await
            .expect("insert active user");
        upsert_user(&pool, "stale-puuid", "stale-token", "1.0.0")
            .await
            .expect("insert stale user");

        sqlx::query(
            "UPDATE users SET last_heartbeat = datetime('now', '-6 minutes') WHERE puuid = $1",
        )
        .bind("stale-puuid")
        .execute(&pool)
        .await
        .expect("age stale user");

        let star_users = query_star_users(
            &pool,
            &["active-puuid".to_string(), "stale-puuid".to_string()],
        )
        .await
        .expect("query star users");

        assert_eq!(star_users, vec!["active-puuid".to_string()]);
        assert_eq!(
            count_active_users(&pool).await.expect("count active users"),
            1
        );
    }
}
