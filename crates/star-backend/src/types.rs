use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub puuid: String,
    pub client_version: String,
}

#[derive(Debug, Serialize)]
pub struct RegisterResponse {
    pub session_token: String,
}

#[derive(Debug, Deserialize)]
pub struct HeartbeatRequest {
    pub session_token: String,
}

#[derive(Debug, Deserialize)]
pub struct DeregisterRequest {
    pub session_token: String,
}

#[derive(Debug, Deserialize)]
pub struct QueryRequest {
    pub puuids: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct QueryResponse {
    pub star_users: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub active_users: i64,
}
