#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub type Puuid = String;

// --- Lockfile ---

#[derive(Debug, Clone)]
pub struct LockfileData {
    pub port: u16,
    pub password: String,
    pub protocol: String,
}

// --- Entitlements ---

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntitlementsResponse {
    pub access_token: String,
    pub entitlements: Vec<serde_json::Value>,
    pub issuer: String,
    pub subject: Puuid,
    pub token: String,
}

// --- Auth context for all API calls ---

#[derive(Debug, Clone)]
pub struct RiotAuth {
    pub puuid: Puuid,
    pub lockfile: LockfileData,
    pub access_token: String,
    pub entitlements_token: String,
    pub region: String,
    pub shard: String,
}

// --- Presence ---

#[derive(Debug, Clone, Deserialize)]
pub struct PresencesResponse {
    pub presences: Vec<Presence>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Presence {
    pub puuid: Puuid,
    pub product: String,
    pub private: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PrivatePresence {
    #[serde(default)]
    pub is_valid: bool,
    #[serde(default)]
    pub session_loop_state: String,
    #[serde(default)]
    pub party_id: String,
    #[serde(default)]
    pub matchmap: String,
    #[serde(default)]
    pub match_id: String,
    #[serde(default, alias = "provisioningFlow")]
    pub provisioning_flow: String,
    #[serde(default)]
    pub queue_id: String,
    #[serde(default)]
    pub party_size: i32,
    #[serde(default)]
    pub party_version: i64,
}

// --- MMR / Rank ---

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct MmrResponse {
    pub version: Option<i64>,
    pub subject: Option<Puuid>,
    pub new_player_experience_finished: Option<bool>,
    pub queue_skills: Option<HashMap<String, QueueSkill>>,
    pub latest_competitive_update: Option<CompetitiveUpdate>,
    pub is_leaderboard_anonymized: Option<bool>,
    pub is_act_rank_badge_hidden: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct QueueSkill {
    pub total_games_needed_for_rating: Option<i32>,
    pub total_games_needed_for_leaderboard: Option<i32>,
    pub current_season_games_needed_for_rating: Option<i32>,
    pub seasonal_info_by_season_i_d: Option<HashMap<String, SeasonalInfo>>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct SeasonalInfo {
    pub season_i_d: Option<String>,
    pub number_of_wins: Option<i32>,
    pub number_of_wins_with_placements: Option<i32>,
    pub number_of_games: Option<i32>,
    pub rank: Option<i32>,
    pub capstone_wins: Option<i32>,
    pub leaderboard_rank: Option<i32>,
    pub competitive_tier: Option<i32>,
    #[serde(alias = "RankedRating")]
    pub ranking_in_tier: Option<i32>,
    pub wins_by_tier: Option<HashMap<String, i32>>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct CompetitiveUpdate {
    pub match_i_d: Option<String>,
    pub map_i_d: Option<String>,
    pub season_i_d: Option<String>,
    pub competitive_tier: Option<i32>,
    pub ranking_in_tier: Option<i32>,
    pub tier_after_update: Option<i32>,
    pub tier_before_update: Option<i32>,
    pub ranked_rating_after_update: Option<i32>,
    pub ranked_rating_before_update: Option<i32>,
    pub ranked_rating_earned: Option<i32>,
    pub ranked_rating_performance_bonus: Option<i32>,
    pub afk_penalty: Option<i32>,
}

// --- Competitive Updates ---

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct CompetitiveUpdatesResponse {
    pub matches: Vec<CompetitiveUpdate>,
    pub subject: Option<Puuid>,
}

// --- Match Details ---

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchDetailsResponse {
    pub match_info: MatchInfo,
    pub players: Vec<MatchPlayer>,
    pub teams: Option<Vec<MatchTeam>>,
    pub round_results: Option<Vec<RoundResult>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchInfo {
    pub match_id: String,
    pub map_id: String,
    pub game_mode: Option<String>,
    pub queue_id: Option<String>,
    pub season_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchPlayer {
    pub subject: Puuid,
    pub team_id: Option<String>,
    pub character_id: Option<String>,
    pub stats: Option<PlayerStats>,
    pub competitive_tier: Option<i32>,
    pub player_card: Option<String>,
    pub player_title: Option<String>,
    pub party_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerStats {
    pub score: Option<i32>,
    pub rounds_played: Option<i32>,
    pub kills: Option<i32>,
    pub deaths: Option<i32>,
    pub assists: Option<i32>,
    pub ability_casts: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchTeam {
    pub team_id: String,
    pub won: bool,
    pub rounds_played: Option<i32>,
    pub rounds_won: Option<i32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoundResult {
    pub round_num: Option<i32>,
    pub player_stats: Option<Vec<RoundPlayerStats>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoundPlayerStats {
    pub subject: Puuid,
    pub damage: Option<Vec<RoundDamage>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoundDamage {
    pub headshots: Option<i32>,
    pub bodyshots: Option<i32>,
    pub legshots: Option<i32>,
}

// --- Pregame ---

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PregamePlayerResponse {
    pub subject: Puuid,
    pub match_i_d: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PregameMatchResponse {
    pub i_d: String,
    pub map_i_d: Option<String>,
    pub mode: Option<String>,
    pub queue_i_d: Option<String>,
    pub provisioning_flow_i_d: Option<String>,
    #[serde(rename = "GamePodID")]
    pub game_pod_id: Option<String>,
    pub ally_team: Option<PregameTeam>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PregameTeam {
    pub team_i_d: Option<String>,
    pub players: Vec<PregamePlayer>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PregamePlayer {
    pub subject: Puuid,
    pub character_i_d: Option<String>,
    pub character_selection_state: Option<String>,
    pub player_identity: Option<PlayerIdentity>,
    pub is_captain: Option<bool>,
}

// --- Coregame ---

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct CoregamePlayerResponse {
    pub subject: Puuid,
    pub match_i_d: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct CoregameMatchResponse {
    pub match_i_d: String,
    pub map_i_d: Option<String>,
    pub mode_i_d: Option<String>,
    pub queue_i_d: Option<String>,
    pub provisioning_flow: Option<String>,
    #[serde(rename = "GamePodID")]
    pub game_pod_id: Option<String>,
    pub players: Vec<CoregamePlayer>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct CoregamePlayer {
    pub subject: Puuid,
    pub team_i_d: Option<String>,
    pub character_i_d: Option<String>,
    pub player_identity: Option<PlayerIdentity>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PlayerIdentity {
    pub subject: Option<Puuid>,
    pub player_card_i_d: Option<String>,
    pub player_title_i_d: Option<String>,
    pub account_level: Option<i32>,
    pub preferred_level_border_i_d: Option<String>,
    pub incognito: Option<bool>,
    pub hide_account_level: Option<bool>,
}

// --- Loadouts ---

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LoadoutsResponse {
    pub loadouts: Vec<PlayerLoadout>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PlayerLoadout {
    pub subject: Puuid,
    pub character_i_d: Option<String>,
    pub loadout: Option<EquippedLoadout>,
    pub items: Option<HashMap<String, LoadoutItem>>,
}

impl PlayerLoadout {
    pub fn items(&self) -> Option<&HashMap<String, LoadoutItem>> {
        self.loadout
            .as_ref()
            .and_then(|loadout| loadout.items.as_ref())
            .or(self.items.as_ref())
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct EquippedLoadout {
    pub items: Option<HashMap<String, LoadoutItem>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LoadoutItem {
    pub i_d: Option<String>,
    pub type_i_d: Option<String>,
    pub sockets: Option<HashMap<String, LoadoutSocket>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LoadoutSocket {
    pub i_d: Option<String>,
    pub item: Option<SocketItem>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct SocketItem {
    pub i_d: Option<String>,
    pub type_i_d: Option<String>,
}

// --- Name Service ---

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct NameServiceEntry {
    pub display_name: Option<String>,
    pub subject: Puuid,
    pub game_name: Option<String>,
    pub tag_line: Option<String>,
}

// --- Content / Seasons ---

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ContentResponse {
    pub seasons: Option<Vec<ContentSeason>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ContentSeason {
    pub i_d: Option<String>,
    pub name: Option<String>,
    pub is_active: Option<bool>,
    #[serde(rename = "Type")]
    pub season_type: Option<String>,
}

// --- valorant-api.com external types ---

#[derive(Debug, Clone, Deserialize)]
pub struct ValorantApiResponse<T> {
    pub status: i32,
    pub data: Option<T>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionData {
    pub riot_client_version: Option<String>,
    pub build_version: Option<String>,
    pub branch: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentData {
    pub uuid: String,
    pub display_name: String,
    pub display_icon: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MapData {
    pub uuid: String,
    pub display_name: String,
    pub map_url: Option<String>,
    pub display_icon: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WeaponData {
    pub uuid: String,
    pub display_name: String,
    #[serde(default)]
    pub skins: Vec<WeaponSkinData>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WeaponSkinData {
    pub uuid: String,
    pub display_name: String,
    pub display_icon: Option<String>,
    pub content_tier_uuid: Option<String>,
    #[serde(default)]
    pub levels: Vec<WeaponSkinLevelData>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WeaponSkinLevelData {
    pub uuid: String,
    pub display_name: Option<String>,
    pub display_icon: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct SkinLevelInfo {
    pub display_name: String,
    pub skin_name: String,
    pub level: usize,
    pub total_levels: usize,
    pub color: egui::Color32,
}

// --- Aggregated player data for overlay display ---

#[derive(Debug, Clone, Default, Serialize)]
pub struct PlayerDisplayData {
    pub puuid: Puuid,
    pub game_name: String,
    pub tag_line: String,
    pub team_id: String,
    pub agent_name: String,
    pub agent_icon: Option<String>,
    pub current_rank: i32,
    pub rank_name: String,
    pub rr: i32,
    pub peak_rank: i32,
    pub peak_rank_name: String,
    pub previous_rank: i32,
    pub previous_rank_name: String,
    pub leaderboard_position: i32,
    pub kd: f64,
    pub headshot_percent: f64,
    pub winrate: f64,
    pub wins: i32,
    pub games: i32,
    pub recent_results: String,
    pub account_level: i32,
    pub skin_name: String,
    pub skin_level: usize,
    pub skin_level_total: usize,
    #[serde(skip)]
    pub skin_color: egui::Color32,
    pub party_id: String,
    pub party_number: i32,
    pub is_incognito: bool,
    pub is_star_user: bool,
    pub times_seen_before: i32,
    pub last_seen_at: String,
    pub last_seen_game_name: String,
    pub last_seen_tag_line: String,
    #[serde(skip)]
    pub enriched: bool,
}

pub const RANK_NAMES: &[&str] = &[
    "Unranked",
    "Unused1",
    "Unused2",
    "Iron 1",
    "Iron 2",
    "Iron 3",
    "Bronze 1",
    "Bronze 2",
    "Bronze 3",
    "Silver 1",
    "Silver 2",
    "Silver 3",
    "Gold 1",
    "Gold 2",
    "Gold 3",
    "Platinum 1",
    "Platinum 2",
    "Platinum 3",
    "Diamond 1",
    "Diamond 2",
    "Diamond 3",
    "Ascendant 1",
    "Ascendant 2",
    "Ascendant 3",
    "Immortal 1",
    "Immortal 2",
    "Immortal 3",
    "Radiant",
];

pub fn rank_name(tier: i32) -> &'static str {
    RANK_NAMES.get(tier as usize).copied().unwrap_or("Unranked")
}

pub fn rank_color(tier: i32) -> egui::Color32 {
    match tier {
        3..=5 => egui::Color32::from_rgb(72, 69, 62),   // Iron
        6..=8 => egui::Color32::from_rgb(187, 143, 90), // Bronze
        9..=11 => egui::Color32::from_rgb(174, 178, 178), // Silver
        12..=14 => egui::Color32::from_rgb(197, 186, 63), // Gold
        15..=17 => egui::Color32::from_rgb(24, 167, 185), // Platinum
        18..=20 => egui::Color32::from_rgb(216, 100, 199), // Diamond
        21..=23 => egui::Color32::from_rgb(24, 148, 82), // Ascendant
        24..=26 => egui::Color32::from_rgb(221, 68, 68), // Immortal
        27 => egui::Color32::from_rgb(255, 253, 205),   // Radiant
        _ => egui::Color32::from_rgb(46, 46, 46),       // Unranked
    }
}

pub fn skin_tier_color(content_tier_uuid: Option<&str>) -> egui::Color32 {
    match content_tier_uuid.map(|value| value.to_ascii_lowercase()) {
        Some(uuid) if uuid == "0cebb8be-46d7-c12a-d306-e9907bfc5a25" => {
            egui::Color32::from_rgb(0, 149, 135)
        }
        Some(uuid) if uuid == "e046854e-406c-37f4-6607-19a9ba8426fc" => {
            egui::Color32::from_rgb(241, 184, 45)
        }
        Some(uuid) if uuid == "60bca009-4182-7998-dee7-b8a2558dc369" => {
            egui::Color32::from_rgb(209, 84, 141)
        }
        Some(uuid) if uuid == "12683d76-48d7-84a3-4e09-6985794f0445" => {
            egui::Color32::from_rgb(90, 159, 226)
        }
        Some(uuid) if uuid == "411e4a55-4e59-7757-41f0-86a53f101bb5" => {
            egui::Color32::from_rgb(239, 235, 101)
        }
        _ => egui::Color32::from_rgb(160, 160, 175),
    }
}

#[cfg(test)]
mod tests {
    use super::{skin_tier_color, LoadoutsResponse};

    #[test]
    fn loadouts_deserialize_nested_game_shape() {
        let json = r#"
        {
          "Loadouts": [
            {
              "Subject": "player-1",
              "CharacterID": "agent-1",
              "Loadout": {
                "Items": {
                  "weapon-1": {
                    "Sockets": {
                      "bcef87d6-209b-46c6-8b19-fbe40bd95abc": {
                        "Item": {
                          "ID": "skin-1"
                        }
                      }
                    }
                  }
                }
              }
            }
          ]
        }"#;

        let parsed: LoadoutsResponse = serde_json::from_str(json).unwrap();
        let loadout = &parsed.loadouts[0];
        let items = loadout.items().unwrap();

        assert!(loadout.character_i_d.as_deref() == Some("agent-1"));
        assert!(items.contains_key("weapon-1"));
    }

    #[test]
    fn loadouts_deserialize_flat_shape() {
        let json = r#"
        {
          "Loadouts": [
            {
              "Subject": "player-1",
              "Items": {
                "weapon-1": {
                  "Sockets": {
                    "bcef87d6-209b-46c6-8b19-fbe40bd95abc": {
                      "Item": {
                        "ID": "skin-1"
                      }
                    }
                  }
                }
              }
            }
          ]
        }"#;

        let parsed: LoadoutsResponse = serde_json::from_str(json).unwrap();
        let items = parsed.loadouts[0].items().unwrap();

        assert!(items.contains_key("weapon-1"));
    }

    #[test]
    fn maps_vry_skin_tiers_to_colors() {
        assert_eq!(
            skin_tier_color(Some("0cebb8be-46d7-c12a-d306-e9907bfc5a25")),
            egui::Color32::from_rgb(0, 149, 135)
        );
        assert_eq!(
            skin_tier_color(Some("411e4a55-4e59-7757-41f0-86a53f101bb5")),
            egui::Color32::from_rgb(239, 235, 101)
        );
        assert_eq!(
            skin_tier_color(None),
            egui::Color32::from_rgb(160, 160, 175)
        );
    }
}
