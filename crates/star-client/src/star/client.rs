use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterRequest {
    pub puuid: String,
    pub client_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterResponse {
    pub session_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatRequest {
    pub session_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeregisterRequest {
    pub session_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryRequest {
    pub puuids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResponse {
    pub star_users: Vec<String>,
}

pub struct StarClient {
    http: reqwest::Client,
    backend_url: String,
    session_token: Arc<RwLock<Option<String>>>,
    heartbeat_active: Arc<AtomicBool>,
}

impl StarClient {
    pub fn new(backend_url: &str) -> Self {
        Self {
            http: reqwest::Client::new(),
            backend_url: backend_url.trim_end_matches('/').to_string(),
            session_token: Arc::new(RwLock::new(None)),
            heartbeat_active: Arc::new(AtomicBool::new(false)),
        }
    }

    pub async fn register(&self, puuid: &str) -> Result<()> {
        let url = format!("{}/api/register", self.backend_url);
        let resp: RegisterResponse = self
            .http
            .post(&url)
            .json(&RegisterRequest {
                puuid: puuid.to_string(),
                client_version: env!("CARGO_PKG_VERSION").to_string(),
            })
            .send()
            .await?
            .json()
            .await?;

        *self.session_token.write().await = Some(resp.session_token);
        tracing::info!("Registered with star backend");
        Ok(())
    }

    pub async fn heartbeat(&self) -> Result<()> {
        let token = self.session_token.read().await.clone();
        if let Some(token) = token {
            let url = format!("{}/api/heartbeat", self.backend_url);
            self.http
                .post(&url)
                .json(&HeartbeatRequest {
                    session_token: token,
                })
                .send()
                .await?;
        }
        Ok(())
    }

    pub async fn deregister(&self) -> Result<()> {
        let token = self.session_token.write().await.take();
        self.heartbeat_active.store(false, Ordering::Relaxed);
        if let Some(token) = token {
            let url = format!("{}/api/deregister", self.backend_url);
            let _ = self
                .http
                .post(&url)
                .json(&DeregisterRequest {
                    session_token: token,
                })
                .send()
                .await;
        }
        Ok(())
    }

    pub async fn query(&self, puuids: &[String]) -> Result<Vec<String>> {
        let url = format!("{}/api/query", self.backend_url);
        let resp: QueryResponse = self
            .http
            .post(&url)
            .json(&QueryRequest {
                puuids: puuids.to_vec(),
            })
            .send()
            .await?
            .json()
            .await?;
        Ok(resp.star_users)
    }

    pub fn start_heartbeat_loop(self: &Arc<Self>) {
        self.heartbeat_active.store(true, Ordering::Relaxed);
        let client = Arc::clone(self);
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                if !client.heartbeat_active.load(Ordering::Relaxed) {
                    tracing::debug!("Heartbeat loop stopped");
                    break;
                }
                if let Err(e) = client.heartbeat().await {
                    tracing::warn!("Heartbeat failed: {}", e);
                }
            }
        });
    }
}
