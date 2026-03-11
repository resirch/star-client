use super::{endpoints, types::*};
use anyhow::Result;
use std::collections::HashMap;

#[derive(Clone)]
pub struct RiotApiClient {
    http: reqwest::Client,
    auth: RiotAuth,
    client_version: String,
    content_cache: Option<ContentResponse>,
    agent_cache: HashMap<String, AgentData>,
    skin_cache: HashMap<String, SkinLevelInfo>,
}

impl RiotApiClient {
    pub fn new(auth: RiotAuth) -> Result<Self> {
        let http = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .build()?;

        Ok(Self {
            http,
            auth,
            client_version: String::new(),
            content_cache: None,
            agent_cache: HashMap::new(),
            skin_cache: HashMap::new(),
        })
    }

    pub fn auth(&self) -> &RiotAuth {
        &self.auth
    }

    pub fn puuid(&self) -> &str {
        &self.auth.puuid
    }

    fn riot_headers(&self) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "Authorization",
            format!("Bearer {}", self.auth.access_token)
                .parse()
                .unwrap(),
        );
        headers.insert(
            "X-Riot-Entitlements-JWT",
            self.auth.entitlements_token.parse().unwrap(),
        );
        headers.insert(
            "X-Riot-ClientPlatform",
            "ew0KCSJwbGF0Zm9ybVR5cGUiOiAiUEMiLA0KCSJwbGF0Zm9ybU9TIjogIldpbmRvd3MiLA0KCSJwbGF0Zm9ybU9TVmVyc2lvbiI6ICIxMC4wLjE5MDQyLjEuMjU2LjY0Yml0IiwNCgkicGxhdGZvcm1DaGlwc2V0IjogIlVua25vd24iDQp9"
                .parse()
                .unwrap(),
        );
        if !self.client_version.is_empty() {
            headers.insert("X-Riot-ClientVersion", self.client_version.parse().unwrap());
        }
        headers
    }

    fn local_headers(&self) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();
        let basic = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            format!("riot:{}", self.auth.lockfile.password),
        );
        headers.insert("Authorization", format!("Basic {}", basic).parse().unwrap());
        headers
    }

    // --- Presence ---

    pub async fn get_presences(&self) -> Result<Vec<Presence>> {
        let url = endpoints::local_presence(self.auth.lockfile.port);
        let resp: PresencesResponse = self
            .http
            .get(&url)
            .headers(self.local_headers())
            .send()
            .await?
            .json()
            .await?;
        Ok(resp.presences)
    }

    pub async fn get_self_presence(&self) -> Result<Option<PrivatePresence>> {
        let presences = self.get_presences().await?;
        for p in presences {
            if p.puuid == self.auth.puuid && p.product == "valorant" {
                if let Some(priv_b64) = &p.private {
                    let decoded = base64::Engine::decode(
                        &base64::engine::general_purpose::STANDARD,
                        priv_b64,
                    )
                    .unwrap_or_default();
                    let parsed: PrivatePresence =
                        serde_json::from_slice(&decoded).unwrap_or_default();
                    return Ok(Some(parsed));
                }
            }
        }
        Ok(None)
    }

    // --- MMR ---

    pub async fn get_mmr(&self, puuid: &str) -> Result<MmrResponse> {
        let url = endpoints::mmr(&self.auth, puuid);
        let response = self
            .http
            .get(&url)
            .headers(self.riot_headers())
            .send()
            .await?;

        let status = response.status();
        let resp_text = response.text().await?;

        if !status.is_success() {
            tracing::warn!(
                "MMR request for {} returned HTTP {}: {}",
                &puuid[..8.min(puuid.len())],
                status.as_u16(),
                &resp_text[..500.min(resp_text.len())]
            );
            return Ok(MmrResponse::default());
        }

        match serde_json::from_str::<MmrResponse>(&resp_text) {
            Ok(mmr) => {
                if mmr.subject.is_none() && mmr.queue_skills.is_none() {
                    tracing::warn!(
                        "MMR response for {} parsed OK but has no data — raw: {}",
                        &puuid[..8.min(puuid.len())],
                        &resp_text[..500.min(resp_text.len())]
                    );
                }
                Ok(mmr)
            }
            Err(e) => {
                tracing::warn!(
                    "MMR deserialization failed for {}: {} — raw: {}",
                    &puuid[..8.min(puuid.len())],
                    e,
                    &resp_text[..500.min(resp_text.len())]
                );
                Ok(MmrResponse::default())
            }
        }
    }

    // --- Competitive Updates ---

    pub async fn get_competitive_updates(&self, puuid: &str) -> Result<CompetitiveUpdatesResponse> {
        let url = endpoints::competitive_updates(&self.auth, puuid);
        let resp = self
            .http
            .get(&url)
            .headers(self.riot_headers())
            .send()
            .await?
            .json()
            .await?;
        Ok(resp)
    }

    // --- Match Details ---

    pub async fn get_match_details(&self, match_id: &str) -> Result<MatchDetailsResponse> {
        let url = endpoints::match_details(&self.auth, match_id);
        let resp = self
            .http
            .get(&url)
            .headers(self.riot_headers())
            .send()
            .await?
            .json()
            .await?;
        Ok(resp)
    }

    // --- Match History ---

    pub async fn get_match_history(&self, puuid: &str) -> Result<serde_json::Value> {
        let url = endpoints::match_history(&self.auth, puuid);
        let resp = self
            .http
            .get(&url)
            .headers(self.riot_headers())
            .send()
            .await?
            .json()
            .await?;
        Ok(resp)
    }

    // --- Name Service ---

    pub async fn get_names(&self, puuids: &[String]) -> Result<Vec<NameServiceEntry>> {
        let url = endpoints::name_service(&self.auth);
        let resp = self
            .http
            .put(&url)
            .headers(self.riot_headers())
            .json(puuids)
            .send()
            .await?
            .json()
            .await?;
        Ok(resp)
    }

    // --- Pregame ---

    pub async fn get_pregame_player(&self) -> Result<PregamePlayerResponse> {
        let url = endpoints::pregame_player(&self.auth, &self.auth.puuid);
        let resp = self
            .http
            .get(&url)
            .headers(self.riot_headers())
            .send()
            .await?
            .json()
            .await?;
        Ok(resp)
    }

    pub async fn get_pregame_match(&self, match_id: &str) -> Result<PregameMatchResponse> {
        let url = endpoints::pregame_match(&self.auth, match_id);
        let resp = self
            .http
            .get(&url)
            .headers(self.riot_headers())
            .send()
            .await?
            .json()
            .await?;
        Ok(resp)
    }

    // --- Coregame ---

    pub async fn get_coregame_player(&self) -> Result<CoregamePlayerResponse> {
        let url = endpoints::coregame_player(&self.auth, &self.auth.puuid);
        let resp = self
            .http
            .get(&url)
            .headers(self.riot_headers())
            .send()
            .await?
            .json()
            .await?;
        Ok(resp)
    }

    pub async fn get_coregame_match(&self, match_id: &str) -> Result<CoregameMatchResponse> {
        let url = endpoints::coregame_match(&self.auth, match_id);
        let resp = self
            .http
            .get(&url)
            .headers(self.riot_headers())
            .send()
            .await?
            .json()
            .await?;
        Ok(resp)
    }

    pub async fn get_coregame_loadouts(&self, match_id: &str) -> Result<LoadoutsResponse> {
        let url = endpoints::coregame_loadouts(&self.auth, match_id);
        let resp = self
            .http
            .get(&url)
            .headers(self.riot_headers())
            .send()
            .await?
            .json()
            .await?;
        Ok(resp)
    }

    // --- Content ---

    pub async fn get_content(&mut self) -> Result<ContentResponse> {
        if let Some(cached) = &self.content_cache {
            return Ok(cached.clone());
        }
        let url = endpoints::content(&self.auth);
        let resp: ContentResponse = self
            .http
            .get(&url)
            .headers(self.riot_headers())
            .send()
            .await?
            .json()
            .await?;
        self.content_cache = Some(resp.clone());
        Ok(resp)
    }

    pub async fn get_current_season_id(&mut self) -> Result<Option<String>> {
        let content = self.get_content().await?;
        if let Some(seasons) = content.seasons {
            for season in &seasons {
                if season.is_active == Some(true) && season.season_type.as_deref() == Some("act") {
                    return Ok(season.i_d.clone());
                }
            }
        }
        Ok(None)
    }

    // --- valorant-api.com ---

    pub async fn fetch_client_version(&mut self) -> Result<()> {
        let resp: ValorantApiResponse<VersionData> = self
            .http
            .get("https://valorant-api.com/v1/version")
            .send()
            .await?
            .json()
            .await?;

        if let Some(data) = resp.data {
            if let Some(version) = data.riot_client_version {
                tracing::info!("Using client version: {}", version);
                self.client_version = version;
                return Ok(());
            }
        }

        tracing::warn!("Could not fetch client version from valorant-api.com");
        Ok(())
    }

    pub async fn fetch_agents(&mut self) -> Result<()> {
        if !self.agent_cache.is_empty() {
            return Ok(());
        }
        let resp: ValorantApiResponse<Vec<AgentData>> = self
            .http
            .get("https://valorant-api.com/v1/agents?isPlayableCharacter=true")
            .send()
            .await?
            .json()
            .await?;
        if let Some(agents) = resp.data {
            for agent in agents {
                self.agent_cache.insert(agent.uuid.to_lowercase(), agent);
            }
        }
        Ok(())
    }

    pub fn get_agent_name(&self, uuid: &str) -> String {
        self.agent_cache
            .get(&uuid.to_lowercase())
            .map(|a| a.display_name.clone())
            .unwrap_or_else(|| "Unknown".into())
    }

    pub async fn fetch_skin_levels(&mut self) -> Result<()> {
        if !self.skin_cache.is_empty() {
            return Ok(());
        }
        let resp: ValorantApiResponse<Vec<WeaponData>> = self
            .http
            .get("https://valorant-api.com/v1/weapons")
            .send()
            .await?
            .json()
            .await?;
        if let Some(weapons) = resp.data {
            for weapon in weapons {
                for skin in weapon.skins {
                    let total_levels = skin.levels.len().max(1);
                    for (index, level) in skin.levels.iter().enumerate() {
                        self.skin_cache.insert(
                            level.uuid.to_lowercase(),
                            SkinLevelInfo {
                                display_name: level
                                    .display_name
                                    .clone()
                                    .unwrap_or_else(|| skin.display_name.clone()),
                                skin_name: skin.display_name.clone(),
                                level: index + 1,
                                total_levels,
                                color: skin_tier_color(skin.content_tier_uuid.as_deref()),
                            },
                        );
                    }
                }
            }
        }
        Ok(())
    }

    pub fn get_skin_name(&self, uuid: &str) -> String {
        self.skin_cache
            .get(&uuid.to_lowercase())
            .map(|skin| skin.display_name.clone())
            .unwrap_or_else(|| "Unknown".into())
    }

    pub fn get_skin_level_info(&self, uuid: &str) -> Option<&SkinLevelInfo> {
        self.skin_cache.get(&uuid.to_lowercase())
    }
}
