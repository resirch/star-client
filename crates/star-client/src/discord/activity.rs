//! Turns Star Client's view of the game into the text and images shown on
//! Discord. Everything here is pure so it can be tested without Discord.

use crate::config::DiscordConfig;
use crate::game::match_data::{is_range_flow, mode_display_name, MatchContext};
use crate::game::state::GameState;
use crate::riot::types::{
    presence_number as number, presence_text as text, rank_name, PlayerDisplayData,
};
use serde_json::Value;

/// The game name Discord shows ("Playing VALORANT"), overriding the
/// Discord application's own name.
pub const GAME_NAME: &str = "VALORANT";

/// Credited in the large image's hover text, which links here.
pub const STAR_CLIENT_NAME: &str = "Star Client";
pub const STAR_CLIENT_URL: &str = "https://github.com/resirch/star-client";

/// Shown in the lobby, and in a match when the map image is off or unknown.
pub const VALORANT_LOGO_URL: &str =
    "https://upload.wikimedia.org/wikipedia/commons/f/fc/Valorant_logo_-_pink_color_version_%28cropped%29.png";

/// Shown instead of the map name and queue in the Range.
const RANGE_NAME: &str = "The Range";

/// Current competitive tier set on valorant-api.com.
const RANK_ICON_BASE: &str =
    "https://media.valorant-api.com/competitivetiers/03621f52-342b-cf4e-4f86-9350a49c6d04";

/// Fields read from the local player's Valorant presence.
///
/// Riot has moved these between the top level and nested objects
/// (`partyPresenceData`, `matchPresenceData`, `playerPresenceData`), so each
/// one is looked up in the nested object first and the top level second.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SelfPresence {
    pub session_loop_state: String,
    pub provisioning_flow: String,
    pub queue_id: String,
    pub party_id: String,
    pub party_state: String,
    pub party_size: i64,
    pub max_party_size: i64,
    pub queue_entry_time: Option<i64>,
    pub custom_game_name: String,
    pub ally_score: i64,
    pub enemy_score: i64,
    pub account_level: i64,
    pub competitive_tier: i64,
    pub leaderboard_position: i64,
}

impl SelfPresence {
    pub fn from_json(value: &Value) -> Self {
        let party = &value["partyPresenceData"];
        let matched = &value["matchPresenceData"];
        let player = &value["playerPresenceData"];

        Self {
            session_loop_state: text(value, matched, "sessionLoopState"),
            provisioning_flow: text(value, matched, "provisioningFlow"),
            queue_id: text(value, matched, "queueId"),
            party_id: text(value, party, "partyId"),
            party_state: text(value, party, "partyState"),
            party_size: number(value, party, "partySize"),
            max_party_size: number(value, party, "maxPartySize"),
            queue_entry_time: parse_queue_entry_time(&text(value, party, "queueEntryTime")),
            custom_game_name: text(value, party, "customGameName"),
            ally_score: number(value, party, "partyOwnerMatchScoreAllyTeam"),
            enemy_score: number(value, party, "partyOwnerMatchScoreEnemyTeam"),
            account_level: number(value, player, "accountLevel"),
            competitive_tier: number(value, player, "competitiveTier"),
            leaderboard_position: number(value, player, "leaderboardPosition"),
        }
    }

    fn in_queue(&self) -> bool {
        self.party_state.eq_ignore_ascii_case("MATCHMAKING")
    }

    fn in_custom_lobby(&self) -> bool {
        self.party_state.eq_ignore_ascii_case("CUSTOM_GAME_SETUP")
    }
}

/// Riot formats this as `2026.11.14-18.02.33` in UTC; the year-1 value means
/// "not queued".
fn parse_queue_entry_time(raw: &str) -> Option<i64> {
    let parsed = chrono::NaiveDateTime::parse_from_str(raw, "%Y.%m.%d-%H.%M.%S").ok()?;
    let timestamp = parsed.and_utc().timestamp();
    (timestamp > 0).then_some(timestamp)
}

/// Everything sent to Discord for one update. Compared between polls so an
/// unchanged activity isn't re-sent.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ActivityView {
    pub details: String,
    /// Makes the details line a link. Unlike buttons, which Discord hides
    /// from the player whose presence it is, this is visible to them too.
    pub details_url: Option<String>,
    pub state: String,
    pub large_image: String,
    pub large_text: String,
    pub small_image: Option<String>,
    pub small_text: Option<String>,
    /// (party id, current size, max size)
    pub party: Option<(String, i32, i32)>,
    pub start_timestamp: Option<i64>,
    /// (label, url)
    pub button: Option<(String, String)>,
}

pub struct ActivityInput<'a> {
    pub game_state: &'a GameState,
    pub context: Option<&'a MatchContext>,
    pub presence: Option<&'a SelfPresence>,
    /// The local player's row from the overlay, if it has been loaded.
    pub player: Option<&'a PlayerDisplayData>,
    pub config: &'a DiscordConfig,
    /// When the current game state began (unix seconds).
    pub state_started_at: i64,
}

/// Returns `None` when nothing should be shown (Valorant isn't running).
pub fn build_activity(input: &ActivityInput) -> Option<ActivityView> {
    let config = input.config;
    let presence = input.presence;

    let in_match_loading = presence.is_some_and(|p| {
        let loop_state = p.session_loop_state.to_ascii_uppercase();
        loop_state == "PREGAME" || loop_state == "INGAME"
    });

    let rank = RankInfo::from_input(input);
    let rank_text = config
        .show_rank
        .then(|| rank.label(config.show_rr))
        .flatten();
    let rank_icon = (config.show_rank && rank.tier > 0)
        .then(|| format!("{}/{}/largeicon.png", RANK_ICON_BASE, rank.tier));

    let agent = input
        .player
        .map(|p| p.agent_name.trim())
        .filter(|name| config.show_agent && !name.is_empty() && *name != "Unknown");
    let agent_icon = agent.and_then(|_| input.player.and_then(|p| p.agent_icon.clone()));

    let in_range = input.context.is_some_and(MatchContext::is_range)
        || presence.is_some_and(|p| is_range_flow(&p.provisioning_flow));
    // Players can queue from the Range; their party's queue is the one they
    // are waiting on.
    let queueing = config.show_queue_status && presence.is_some_and(SelfPresence::in_queue);
    let presence_queue_id = presence.map(|p| p.queue_id.as_str()).unwrap_or_default();

    let queue_id = input
        .context
        .map(|c| c.queue.as_str())
        .filter(|q| !q.is_empty())
        .or_else(|| presence.map(|p| p.queue_id.as_str()))
        .unwrap_or_default();
    let mode = config
        .show_mode
        .then(|| display_mode(queue_id, presence))
        .flatten();

    let map_name = input
        .context
        .map(|c| c.map.name.as_str())
        .filter(|name| !name.is_empty() && *name != "Unknown");
    let map_image = input
        .context
        .and_then(|c| c.map.splash.clone())
        .filter(|_| config.show_map_image);

    let mut view = ActivityView {
        large_image: VALORANT_LOGO_URL.to_string(),
        large_text: GAME_NAME.to_string(),
        start_timestamp: config.show_elapsed_time.then_some(input.state_started_at),
        ..Default::default()
    };

    match input.game_state {
        // Without a fresh presence the client is likely closed; the game
        // state alone can be left over from before.
        GameState::Menu if presence.is_none() => return None,
        GameState::Menu => {
            view.details = if queueing {
                "In Queue".to_string()
            } else if presence.is_some_and(SelfPresence::in_custom_lobby) {
                "Custom Game Lobby".to_string()
            } else {
                "In Lobby".to_string()
            };
            if queueing {
                if let Some(entered) = presence.and_then(|p| p.queue_entry_time) {
                    view.start_timestamp = Some(entered);
                }
            }
            let level = presence
                .map(|p| p.account_level)
                .filter(|level| config.show_account_level && *level > 0)
                .map(|level| format!("Level {level}"));
            view.state = join([mode, rank_text.clone(), level]);
            set_small_image(&mut view, rank_icon, rank_text);
        }
        GameState::Pregame { .. } => {
            view.details = match map_name.filter(|_| config.show_map_name) {
                Some(map) => format!("Agent Select - {map}"),
                None => "Agent Select".to_string(),
            };
            view.state = join([mode, agent.map(str::to_string), rank_text.clone()]);
            apply_map_image(&mut view, map_image, map_name);
            match agent_icon {
                Some(icon) => set_small_image(&mut view, Some(icon), agent.map(str::to_string)),
                None => set_small_image(&mut view, rank_icon, rank_text),
            }
        }
        GameState::Ingame { .. } if in_range => {
            let agent = agent.map(str::to_string);
            if queueing {
                let queue_mode = config
                    .show_mode
                    .then(|| display_mode(presence_queue_id, None))
                    .flatten();
                view.details = join([Some("In Queue".to_string()), queue_mode]);
                view.state = join([
                    Some(RANGE_NAME.to_string()),
                    agent.clone(),
                    rank_text.clone(),
                ]);
                if let Some(entered) = presence.and_then(|p| p.queue_entry_time) {
                    view.start_timestamp = Some(entered);
                }
            } else {
                view.details = RANGE_NAME.to_string();
                view.state = join([agent.clone(), rank_text.clone()]);
            }
            apply_map_image(&mut view, map_image, Some(RANGE_NAME));
            match agent_icon {
                Some(icon) => set_small_image(&mut view, Some(icon), agent),
                None => set_small_image(&mut view, rank_icon, rank_text),
            }
        }
        GameState::Ingame { .. } => {
            let map = map_name
                .filter(|_| config.show_map_name)
                .map(str::to_string);
            view.details = match join([mode.clone(), map]) {
                details if details.is_empty() => "In Game".to_string(),
                details => details,
            };
            let score = presence
                .filter(|_| config.show_score && has_team_score(queue_id))
                .map(|p| format!("{} - {}", p.ally_score, p.enemy_score));
            view.state = join([score, agent.map(str::to_string), rank_text.clone()]);
            apply_map_image(&mut view, map_image, map_name);
            match agent_icon {
                Some(icon) => set_small_image(&mut view, Some(icon), agent.map(str::to_string)),
                None => set_small_image(&mut view, rank_icon, rank_text),
            }
        }
        GameState::WaitingForClient => {
            // Between the lobby and a match the client briefly has no match
            // ID; anything else means Valorant isn't running.
            if !in_match_loading {
                return None;
            }
            view.details = if in_range {
                format!("Loading Into {RANGE_NAME}")
            } else {
                "Loading Into Match".to_string()
            };
            // The Range has no mode; the party's queue isn't what's loading.
            view.state = join([mode.filter(|_| !in_range), rank_text.clone()]);
            set_small_image(&mut view, rank_icon, rank_text);
        }
    }

    if config.show_party_size {
        if let Some(p) = presence.filter(|p| p.party_size > 0 && p.max_party_size > 0) {
            let id = if p.party_id.is_empty() {
                "star-client-party".to_string()
            } else {
                p.party_id.clone()
            };
            view.party = Some((id, p.party_size as i32, p.max_party_size as i32));
        }
    }

    if config.show_profile_button {
        if let Some(player) = input
            .player
            .filter(|p| !p.game_name.is_empty() && !p.tag_line.is_empty() && !p.is_incognito)
        {
            let riot_id = format!("{}#{}", player.game_name, player.tag_line);
            let url = format!(
                "https://tracker.gg/valorant/profile/riot/{}/overview",
                percent_encode(&riot_id)
            );
            view.details_url = Some(url.clone());
            view.button = Some(("Tracker.gg Profile".to_string(), url));
        }
    }

    // Discord rejects details/state shorter than 2 characters, and the party
    // size is only rendered next to a non-empty state.
    if view.state.chars().count() < 2 {
        view.state = "VALORANT".to_string();
    }
    view.large_text = format!("{} | {}", view.large_text, STAR_CLIENT_NAME);

    view.details = truncate(&view.details, 128);
    view.state = truncate(&view.state, 128);

    Some(view)
}

struct RankInfo {
    tier: i32,
    name: String,
    rr: Option<i32>,
    leaderboard: i32,
}

impl RankInfo {
    fn from_input(input: &ActivityInput) -> Self {
        // The overlay's enriched row is the freshest source (it has RR); the
        // presence tier covers the lobby before that row exists.
        if let Some(player) = input.player.filter(|p| p.current_rank > 0) {
            return Self {
                tier: player.current_rank,
                name: roman_rank_name(player.current_rank),
                rr: Some(player.rr),
                leaderboard: player.leaderboard_position,
            };
        }
        let presence = input.presence;
        let tier = presence
            .map(|p| p.competitive_tier as i32)
            .unwrap_or_default();
        Self {
            tier,
            name: roman_rank_name(tier),
            rr: None,
            leaderboard: presence
                .map(|p| p.leaderboard_position as i32)
                .unwrap_or_default(),
        }
    }

    fn label(&self, show_rr: bool) -> Option<String> {
        if self.tier <= 0 {
            return Some("Unranked".to_string());
        }
        let mut label = self.name.clone();
        if show_rr {
            if let Some(rr) = self.rr {
                label.push_str(&format!(" - {rr} RR"));
            }
        }
        if self.leaderboard > 0 {
            label.push_str(&format!(" #{}", self.leaderboard));
        }
        Some(label)
    }
}

/// "Diamond 2" becomes "Diamond II"; names without a division are unchanged.
fn roman_rank_name(tier: i32) -> String {
    let name = rank_name(tier);
    let roman = |division| match division {
        "1" => Some("I"),
        "2" => Some("II"),
        "3" => Some("III"),
        _ => None,
    };
    match name.rsplit_once(' ') {
        Some((base, division)) => match roman(division) {
            Some(numeral) => format!("{base} {numeral}"),
            None => name.to_string(),
        },
        None => name.to_string(),
    }
}

fn display_mode(queue_id: &str, presence: Option<&SelfPresence>) -> Option<String> {
    if presence.is_some_and(SelfPresence::in_custom_lobby) {
        return Some(match presence.map(|p| p.custom_game_name.trim()) {
            Some(name) if !name.is_empty() => format!("Custom: {name}"),
            _ => "Custom Game".to_string(),
        });
    }
    if queue_id.is_empty() {
        return None;
    }
    match mode_display_name(queue_id) {
        "Unknown" => None,
        name => Some(name.to_string()),
    }
}

/// Free-for-all Deathmatch has no team score.
fn has_team_score(queue_id: &str) -> bool {
    !queue_id.eq_ignore_ascii_case("deathmatch")
}

fn apply_map_image(view: &mut ActivityView, image: Option<String>, map_name: Option<&str>) {
    if let Some(image) = image {
        view.large_image = image;
    }
    if let Some(map) = map_name {
        view.large_text = map.to_string();
    }
}

fn set_small_image(view: &mut ActivityView, image: Option<String>, text: Option<String>) {
    if let Some(image) = image {
        view.small_image = Some(image);
        view.small_text = text;
    }
}

fn join<const N: usize>(parts: [Option<String>; N]) -> String {
    parts
        .into_iter()
        .flatten()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" | ")
}

fn truncate(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut cut: String = text.chars().take(max_chars - 3).collect();
    cut.push_str("...");
    cut
}

fn percent_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::match_data::MapInfo;
    use serde_json::json;

    fn current_presence_json() -> Value {
        // Shape observed from the live client (release-13.10).
        json!({
            "isValid": true,
            "matchPresenceData": {
                "matchMap": "",
                "provisioningFlow": "Invalid",
                "queueId": "competitive",
                "sessionLoopState": "MENUS"
            },
            "maxPartySize": 5,
            "partyId": "party-1",
            "partyOwnerMatchScoreAllyTeam": 0,
            "partyOwnerMatchScoreEnemyTeam": 0,
            "partyPresenceData": {
                "customGameName": "",
                "maxPartySize": 5,
                "partyId": "party-1",
                "partyOwnerMatchScoreAllyTeam": 7,
                "partyOwnerMatchScoreEnemyTeam": 5,
                "partySize": 2,
                "partyState": "MATCHMAKING",
                "queueEntryTime": "2026.11.14-18.02.33"
            },
            "partySize": 2,
            "playerPresenceData": {
                "accountLevel": 379,
                "competitiveTier": 18,
                "leaderboardPosition": 0
            },
            "queueId": "competitive"
        })
    }

    fn player() -> PlayerDisplayData {
        PlayerDisplayData {
            game_name: "Star Player".into(),
            tag_line: "NA1".into(),
            agent_name: "Jett".into(),
            agent_icon: Some("https://example.com/jett.png".into()),
            current_rank: 19,
            rr: 42,
            ..Default::default()
        }
    }

    fn context() -> MatchContext {
        MatchContext {
            map: MapInfo {
                name: "Ascent".into(),
                splash: Some("https://example.com/ascent.png".into()),
            },
            queue: "competitive".into(),
            mode: String::new(),
            server_id: String::new(),
            provisioning_flow: "Matchmaking".into(),
        }
    }

    fn range_context() -> MatchContext {
        MatchContext {
            map: MapInfo {
                name: "The Range".into(),
                splash: Some("https://example.com/range.png".into()),
            },
            queue: String::new(),
            mode: String::new(),
            server_id: String::new(),
            provisioning_flow: "ShootingRange".into(),
        }
    }

    fn range_view(presence: &SelfPresence) -> ActivityView {
        let config = DiscordConfig::default();
        let player = player();
        let context = range_context();
        build_activity(&ActivityInput {
            game_state: &GameState::Ingame {
                match_id: "range".into(),
            },
            context: Some(&context),
            presence: Some(presence),
            player: Some(&player),
            config: &config,
            state_started_at: 100,
        })
        .unwrap()
    }

    #[test]
    fn range_shows_queue_without_score() {
        let mut presence = SelfPresence::from_json(&current_presence_json());
        presence.session_loop_state = "INGAME".into();
        let view = range_view(&presence);

        assert_eq!(view.details, "In Queue | Competitive");
        assert_eq!(view.state, "The Range | Jett | Diamond II - 42 RR");
        assert_eq!(view.start_timestamp, Some(1_794_679_353));
        assert_eq!(view.large_image, "https://example.com/range.png");
        assert_eq!(view.large_text, "The Range | Star Client");
    }

    #[test]
    fn range_without_queue_shows_agent_and_rank_only() {
        let mut presence = SelfPresence::from_json(&current_presence_json());
        presence.session_loop_state = "INGAME".into();
        presence.party_state = "DEFAULT".into();
        let view = range_view(&presence);

        assert_eq!(view.details, "The Range");
        assert_eq!(view.state, "Jett | Diamond II - 42 RR");
        assert_eq!(view.start_timestamp, Some(100));
    }

    #[test]
    fn parses_nested_presence_fields() {
        let presence = SelfPresence::from_json(&current_presence_json());
        assert_eq!(presence.session_loop_state, "MENUS");
        assert_eq!(presence.party_size, 2);
        assert_eq!(presence.max_party_size, 5);
        assert_eq!(presence.ally_score, 7);
        assert_eq!(presence.enemy_score, 5);
        assert_eq!(presence.account_level, 379);
        assert_eq!(presence.competitive_tier, 18);
        assert_eq!(presence.queue_entry_time, Some(1_794_679_353));
    }

    #[test]
    fn year_one_queue_time_means_not_queued() {
        assert_eq!(parse_queue_entry_time("0001.01.01-00.00.00"), None);
    }

    #[test]
    fn lobby_shows_logo_rank_party_and_queue_timer() {
        let presence = SelfPresence::from_json(&current_presence_json());
        let config = DiscordConfig::default();
        let view = build_activity(&ActivityInput {
            game_state: &GameState::Menu,
            context: None,
            presence: Some(&presence),
            player: None,
            config: &config,
            state_started_at: 100,
        })
        .unwrap();

        assert_eq!(view.details, "In Queue");
        assert_eq!(view.state, "Competitive | Diamond I | Level 379");
        assert_eq!(view.large_image, VALORANT_LOGO_URL);
        assert_eq!(view.large_text, "VALORANT | Star Client");
        assert_eq!(view.party, Some(("party-1".into(), 2, 5)));
        assert_eq!(view.start_timestamp, Some(1_794_679_353));
        assert!(view.small_image.unwrap().ends_with("/18/largeicon.png"));
    }

    #[test]
    fn ingame_shows_map_splash_score_agent_and_rr() {
        let presence = SelfPresence::from_json(&current_presence_json());
        let config = DiscordConfig::default();
        let player = player();
        let context = context();
        let view = build_activity(&ActivityInput {
            game_state: &GameState::Ingame {
                match_id: "m".into(),
            },
            context: Some(&context),
            presence: Some(&presence),
            player: Some(&player),
            config: &config,
            state_started_at: 100,
        })
        .unwrap();

        assert_eq!(view.details, "Competitive | Ascent");
        assert_eq!(view.state, "7 - 5 | Jett | Diamond II - 42 RR");
        assert_eq!(view.large_image, "https://example.com/ascent.png");
        assert_eq!(view.large_text, "Ascent | Star Client");
        assert_eq!(view.small_text.as_deref(), Some("Jett"));
        assert_eq!(view.start_timestamp, Some(100));
        let profile = "https://tracker.gg/valorant/profile/riot/Star%20Player%23NA1/overview";
        assert_eq!(view.button.unwrap().1, profile);
        assert_eq!(view.details_url.as_deref(), Some(profile));
    }

    #[test]
    fn toggles_hide_their_parts() {
        let presence = SelfPresence::from_json(&current_presence_json());
        let config = DiscordConfig {
            show_map_image: false,
            show_mode: false,
            show_score: false,
            show_rank: false,
            show_agent: false,
            show_party_size: false,
            show_elapsed_time: false,
            show_profile_button: false,
            ..DiscordConfig::default()
        };
        let player = player();
        let context = context();
        let view = build_activity(&ActivityInput {
            game_state: &GameState::Ingame {
                match_id: "m".into(),
            },
            context: Some(&context),
            presence: Some(&presence),
            player: Some(&player),
            config: &config,
            state_started_at: 100,
        })
        .unwrap();

        assert_eq!(view.details, "Ascent");
        assert_eq!(view.state, "VALORANT");
        assert_eq!(view.large_image, VALORANT_LOGO_URL);
        assert_eq!(view.small_image, None);
        assert_eq!(view.party, None);
        assert_eq!(view.start_timestamp, None);
        assert_eq!(view.button, None);
        assert_eq!(view.details_url, None);
    }

    #[test]
    fn rank_divisions_use_roman_numerals() {
        assert_eq!(roman_rank_name(3), "Iron I");
        assert_eq!(roman_rank_name(20), "Diamond III");
        assert_eq!(roman_rank_name(27), "Radiant");
        assert_eq!(roman_rank_name(0), "Unranked");
    }

    #[test]
    fn nothing_shown_when_valorant_is_not_running() {
        let config = DiscordConfig::default();
        assert!(build_activity(&ActivityInput {
            game_state: &GameState::WaitingForClient,
            context: None,
            presence: None,
            player: None,
            config: &config,
            state_started_at: 0,
        })
        .is_none());
    }
}
