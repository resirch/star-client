use crate::db;
use crate::types::*;
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use sqlx::SqlitePool;

pub async fn register(
    State(pool): State<SqlitePool>,
    Json(req): Json<RegisterRequest>,
) -> Result<Json<RegisterResponse>, StatusCode> {
    if req.puuid.is_empty() || req.puuid.len() > 128 {
        return Err(StatusCode::BAD_REQUEST);
    }

    let session_token = uuid::Uuid::new_v4().to_string();

    db::upsert_user(&pool, &req.puuid, &session_token, &req.client_version)
        .await
        .map_err(|e| {
            tracing::error!("Register failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    tracing::info!("Registered user: {}...", &req.puuid[..8.min(req.puuid.len())]);

    Ok(Json(RegisterResponse { session_token }))
}

pub async fn heartbeat(
    State(pool): State<SqlitePool>,
    Json(req): Json<HeartbeatRequest>,
) -> StatusCode {
    if req.session_token.is_empty() {
        return StatusCode::BAD_REQUEST;
    }

    match db::update_heartbeat(&pool, &req.session_token).await {
        Ok(true) => StatusCode::OK,
        Ok(false) => StatusCode::NOT_FOUND,
        Err(e) => {
            tracing::error!("Heartbeat failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

pub async fn query(
    State(pool): State<SqlitePool>,
    Json(req): Json<QueryRequest>,
) -> Result<Json<QueryResponse>, StatusCode> {
    if req.puuids.len() > 20 {
        return Err(StatusCode::BAD_REQUEST);
    }

    let star_users = db::query_star_users(&pool, &req.puuids)
        .await
        .map_err(|e| {
            tracing::error!("Query failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(QueryResponse { star_users }))
}

pub async fn health(
    State(pool): State<SqlitePool>,
) -> Json<HealthResponse> {
    let active = db::count_active_users(&pool).await.unwrap_or(0);
    Json(HealthResponse {
        status: "ok".into(),
        active_users: active,
    })
}
