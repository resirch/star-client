use crate::config::Config;
use crate::discord::rpc::DiscordRpc;
use crate::game::history::{EncounterRecord, PlayerHistory};
use crate::game::match_data::{self, MatchContext};
use crate::game::party;
use crate::game::players;
use crate::game::state::{self, GameState};
use crate::riot::api::RiotApiClient;
use crate::riot::types::PlayerDisplayData;
use crate::star::client::StarClient;
use crate::star::presence;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct AppState {
    pub config: Config,
    pub game_state: GameState,
    pub players: Vec<PlayerDisplayData>,
    pub match_context: Option<MatchContext>,
    pub auto_visible: bool,
    pub last_match_id: String,
    pub local_puuid: String,
}

const WAITING_FOR_CLIENT_DEBOUNCE_POLLS: u8 = 3;

impl AppState {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            game_state: GameState::WaitingForClient,
            players: Vec::new(),
            match_context: None,
            auto_visible: false,
            last_match_id: String::new(),
            local_puuid: String::new(),
        }
    }
}

pub async fn run_data_loop(
    app_state: Arc<RwLock<AppState>>,
    api: Arc<RwLock<RiotApiClient>>,
    star_client: Arc<StarClient>,
    quit_flag: Arc<AtomicBool>,
) {
    let history = {
        let data_dir = Config::data_dir();
        PlayerHistory::open(&data_dir).ok()
    };

    let mut discord = DiscordRpc::new();
    let mut discord_rpc_enabled = {
        let state = app_state.read().await;
        state.config.behavior.discord_rpc
    };
    if discord_rpc_enabled {
        discord.connect();
    }

    let mut poll_interval = tokio::time::interval(std::time::Duration::from_secs(5));
    let mut waiting_for_client_polls = 0u8;
    let mut selected_overlay_weapon = {
        let state = app_state.read().await;
        players::normalize_overlay_weapon(&state.config.overlay.weapon).to_string()
    };

    loop {
        poll_interval.tick().await;

        if quit_flag.load(Ordering::Relaxed) {
            discord.disconnect();
            break;
        }

        let mut api_guard = api.write().await;
        let detected_state = state::detect_game_state(&api_guard)
            .await
            .unwrap_or(GameState::Menu);
        let previous_state = {
            let state = app_state.read().await;
            state.game_state.clone()
        };
        let new_state = stabilize_game_state(
            &previous_state,
            detected_state,
            &mut waiting_for_client_polls,
        );

        let config = {
            let state = app_state.read().await;
            state.config.clone()
        };
        let next_overlay_weapon = players::normalize_overlay_weapon(&config.overlay.weapon);
        let overlay_weapon_changed = next_overlay_weapon != selected_overlay_weapon;
        if overlay_weapon_changed {
            selected_overlay_weapon = next_overlay_weapon.to_string();
        }

        if config.behavior.discord_rpc != discord_rpc_enabled {
            discord_rpc_enabled = config.behavior.discord_rpc;
            if discord_rpc_enabled {
                discord.connect();
            } else {
                discord.clear();
                discord.disconnect();
            }
        }

        let state_changed;
        let match_id;
        {
            let state = app_state.read().await;
            state_changed = new_state != state.game_state;
            match_id = match &new_state {
                GameState::Pregame { match_id } | GameState::Ingame { match_id } => {
                    match_id.clone()
                }
                _ => String::new(),
            };
        }

        if state_changed {
            let mut state = app_state.write().await;
            state.game_state = new_state.clone();

            match &new_state {
                GameState::Pregame { .. } if config.behavior.auto_show_pregame => {
                    state.auto_visible = true;
                }
                GameState::Ingame { .. } if config.behavior.auto_hide_ingame => {
                    state.auto_visible = false;
                }
                GameState::Menu | GameState::WaitingForClient => {
                    state.auto_visible = false;
                    state.players.clear();
                    state.match_context = None;
                    state.last_match_id.clear();
                }
                _ => {}
            }
        }

        let is_new_match = {
            let state = app_state.read().await;
            !match_id.is_empty() && match_id != state.last_match_id
        };

        if is_new_match
            || (state_changed && !match_id.is_empty())
            || (overlay_weapon_changed && !match_id.is_empty())
        {
            // Phase 1: Fetch basic player info (names, agents, levels, teams)
            tracing::debug!(
                "Player retrieval triggered: state_changed={}, is_new_match={}, overlay_weapon_changed={}, game_state={:?}, match_id={}",
                state_changed,
                is_new_match,
                overlay_weapon_changed,
                new_state,
                match_id
            );
            let mut players_data = match &new_state {
                GameState::Pregame { match_id } => {
                    players::fetch_pregame_players(&mut api_guard, match_id, &config)
                        .await
                        .unwrap_or_default()
                }
                GameState::Ingame { match_id } => {
                    players::fetch_coregame_players(&mut api_guard, match_id, &config)
                        .await
                        .unwrap_or_default()
                }
                _ => Vec::new(),
            };

            tracing::debug!(
                "Phase 1 complete: fetched {} basic players",
                players_data.len()
            );
            if config.behavior.party_finder && !players_data.is_empty() {
                party::detect_parties(&api_guard, &mut players_data).await;
            }

            if config.star.enabled {
                presence::mark_star_users(&star_client, &mut players_data).await;
            }

            let local_puuid = {
                let state = app_state.read().await;
                state.local_puuid.clone()
            };
            let existing_players_by_puuid: HashMap<String, PlayerDisplayData> = {
                let state = app_state.read().await;
                if state.last_match_id == match_id {
                    state
                        .players
                        .iter()
                        .cloned()
                        .map(|player| (player.puuid.clone(), player))
                        .collect()
                } else {
                    HashMap::new()
                }
            };
            if let Some(history) = &history {
                for player in &mut players_data {
                    hydrate_player_history(
                        player,
                        &local_puuid,
                        &existing_players_by_puuid,
                        history.encounter(&player.puuid),
                    );
                }

                for player in &players_data {
                    if player.puuid == local_puuid {
                        continue;
                    }

                    let update_identity = !player.game_name.is_empty();
                    if is_new_match {
                        let _ = history.record_encounter(
                            &player.puuid,
                            &player.game_name,
                            &player.tag_line,
                            update_identity,
                        );
                        continue;
                    }

                    if should_refresh_encounter_identity(
                        player,
                        existing_players_by_puuid.get(&player.puuid),
                    ) {
                        let _ = history.update_identity(
                            &player.puuid,
                            &player.game_name,
                            &player.tag_line,
                        );
                    }
                }
            }

            let ctx = match &new_state {
                GameState::Pregame { match_id } => {
                    match_data::fetch_pregame_context(&api_guard, match_id)
                        .await
                        .ok()
                }
                GameState::Ingame { match_id } => {
                    match_data::fetch_coregame_context(&api_guard, match_id)
                        .await
                        .ok()
                }
                _ => None,
            };

            // Show basic info immediately
            {
                let mut state = app_state.write().await;
                state.players = players_data;
                state.match_context = ctx;
                state.last_match_id = match_id.clone();
            }

            // Phase 2: Enrich each player with rank/KD/HS% and update after each
            let current_season = api_guard.get_current_season_id().await.ok().flatten();
            let season_lookup = api_guard
                .get_content()
                .await
                .ok()
                .map(|content| players::build_season_lookup(&content))
                .unwrap_or_default();
            tracing::debug!(
                "Phase 2 start: current_season={}",
                current_season.as_deref().unwrap_or("none")
            );
            let player_count = {
                let state = app_state.read().await;
                state.players.len()
            };

            for i in 0..player_count {
                let mut player = {
                    let state = app_state.read().await;
                    if i >= state.players.len() {
                        break;
                    }
                    state.players[i].clone()
                };

                let short_id = &player.puuid[..8.min(player.puuid.len())];
                tracing::debug!(
                    "Phase 2 enrich [{} / {}]: {} '{}#{}'",
                    i + 1,
                    player_count,
                    short_id,
                    player.game_name,
                    player.tag_line
                );
                players::enrich_player(&api_guard, &mut player, &current_season, &season_lookup)
                    .await;

                let mut state = app_state.write().await;
                if i < state.players.len() && state.players[i].puuid == player.puuid {
                    state.players[i] = player;
                }
            }

            let enriched_count = {
                let state = app_state.read().await;
                state.players.iter().filter(|p| p.enriched).count()
            };
            tracing::info!(
                "Initial enrichment: {}/{} players complete",
                enriched_count,
                player_count
            );
        } else if let GameState::Pregame { match_id } = &new_state {
            if let Ok(refreshed_players) =
                players::fetch_pregame_players(&mut api_guard, match_id, &config).await
            {
                let mut state = app_state.write().await;
                if state.last_match_id == *match_id && !state.players.is_empty() {
                    merge_pregame_players(&mut state.players, refreshed_players);
                }
            }
        }

        // Phase 3: Re-enrich players that failed on previous attempts
        if !match_id.is_empty() {
            let unenriched: Vec<(usize, String)> = {
                let state = app_state.read().await;
                if state.last_match_id != match_id {
                    Vec::new()
                } else {
                    state
                        .players
                        .iter()
                        .enumerate()
                        .filter(|(_, p)| !p.enriched && !p.puuid.is_empty())
                        .map(|(i, p)| (i, p.puuid.clone()))
                        .collect()
                }
            };

            if !unenriched.is_empty() {
                tracing::debug!("Re-enriching {} incomplete players", unenriched.len());
                let current_season = api_guard.get_current_season_id().await.ok().flatten();
                let season_lookup = api_guard
                    .get_content()
                    .await
                    .ok()
                    .map(|content| players::build_season_lookup(&content))
                    .unwrap_or_default();

                for (idx, puuid) in &unenriched {
                    let mut player = {
                        let state = app_state.read().await;
                        if *idx >= state.players.len() {
                            continue;
                        }
                        state.players[*idx].clone()
                    };

                    players::enrich_player(
                        &api_guard,
                        &mut player,
                        &current_season,
                        &season_lookup,
                    )
                    .await;

                    let mut state = app_state.write().await;
                    if *idx < state.players.len() && state.players[*idx].puuid == *puuid {
                        state.players[*idx] = player;
                    }
                }
            }
        }

        if discord_rpc_enabled {
            let state = app_state.read().await;
            let rank_name = state
                .players
                .first()
                .map(|p| p.rank_name.as_str())
                .unwrap_or("Unranked");
            let agent_name = state
                .players
                .first()
                .map(|p| p.agent_name.as_str())
                .unwrap_or("");
            discord.update(
                &state.game_state,
                state.match_context.as_ref(),
                rank_name,
                agent_name,
            );
        }
    }
}

fn merge_pregame_players(existing: &mut Vec<PlayerDisplayData>, refreshed: Vec<PlayerDisplayData>) {
    let mut existing_by_puuid: HashMap<String, PlayerDisplayData> = existing
        .drain(..)
        .map(|player| (player.puuid.clone(), player))
        .collect();
    let mut merged = Vec::with_capacity(refreshed.len());

    for latest in refreshed {
        if let Some(mut current) = existing_by_puuid.remove(&latest.puuid) {
            if !latest.game_name.is_empty() {
                current.game_name = latest.game_name;
            }
            if !latest.tag_line.is_empty() {
                current.tag_line = latest.tag_line;
            }
            if !latest.team_id.is_empty() {
                current.team_id = latest.team_id;
            }
            current.agent_name = latest.agent_name;
            current.agent_icon = latest.agent_icon;
            if latest.account_level > 0 {
                current.account_level = latest.account_level;
            }
            current.is_incognito = latest.is_incognito;
            merged.push(current);
        } else {
            merged.push(latest);
        }
    }

    *existing = merged;
}

fn hydrate_player_history(
    player: &mut PlayerDisplayData,
    local_puuid: &str,
    existing_players_by_puuid: &HashMap<String, PlayerDisplayData>,
    encounter: Option<EncounterRecord>,
) {
    if player.puuid == local_puuid {
        return;
    }

    if let Some(existing_player) = existing_players_by_puuid.get(&player.puuid) {
        player.times_seen_before = existing_player.times_seen_before;
        player.last_seen_at = existing_player.last_seen_at.clone();
        player.last_seen_game_name = existing_player.last_seen_game_name.clone();
        player.last_seen_tag_line = existing_player.last_seen_tag_line.clone();
    } else if let Some(encounter) = encounter {
        player.times_seen_before = encounter.times_seen;
        player.last_seen_at = encounter.last_seen_at;
        player.last_seen_game_name = encounter.game_name;
        player.last_seen_tag_line = encounter.tag_line;
    }
}

fn should_refresh_encounter_identity(
    player: &PlayerDisplayData,
    existing_player: Option<&PlayerDisplayData>,
) -> bool {
    if player.game_name.is_empty() {
        return false;
    }

    match existing_player {
        Some(existing_player) => {
            existing_player.game_name != player.game_name
                || existing_player.tag_line != player.tag_line
        }
        None => true,
    }
}

fn stabilize_game_state(
    previous_state: &GameState,
    detected_state: GameState,
    waiting_for_client_polls: &mut u8,
) -> GameState {
    if detected_state == GameState::WaitingForClient
        && previous_state != &GameState::WaitingForClient
    {
        *waiting_for_client_polls = waiting_for_client_polls.saturating_add(1);
        if *waiting_for_client_polls < WAITING_FOR_CLIENT_DEBOUNCE_POLLS {
            return previous_state.clone();
        }
    } else {
        *waiting_for_client_polls = 0;
    }

    detected_state
}

#[cfg(test)]
mod tests {
    use super::{
        hydrate_player_history, merge_pregame_players, should_refresh_encounter_identity,
        stabilize_game_state, WAITING_FOR_CLIENT_DEBOUNCE_POLLS,
    };
    use crate::game::history::EncounterRecord;
    use crate::game::state::GameState;
    use crate::riot::types::PlayerDisplayData;
    use std::collections::HashMap;

    #[test]
    fn pregame_merge_updates_agent_name_without_dropping_enriched_fields() {
        let mut existing = vec![PlayerDisplayData {
            puuid: "player-1".into(),
            game_name: "OldName".into(),
            tag_line: "NA1".into(),
            team_id: "Blue".into(),
            agent_name: String::new(),
            rank_name: "Ascendant 1".into(),
            current_rank: 21,
            rr: 55,
            party_number: 2,
            is_star_user: true,
            enriched: true,
            ..Default::default()
        }];

        let refreshed = vec![PlayerDisplayData {
            puuid: "player-1".into(),
            game_name: "NewName".into(),
            tag_line: "NA1".into(),
            team_id: "Blue".into(),
            agent_name: "Jett".into(),
            ..Default::default()
        }];

        merge_pregame_players(&mut existing, refreshed);

        assert_eq!(existing.len(), 1);
        assert_eq!(existing[0].game_name, "NewName");
        assert_eq!(existing[0].agent_name, "Jett");
        assert_eq!(existing[0].rank_name, "Ascendant 1");
        assert_eq!(existing[0].rr, 55);
        assert_eq!(existing[0].party_number, 2);
        assert!(existing[0].is_star_user);
        assert!(existing[0].enriched);
    }

    #[test]
    fn hydrate_player_history_prefers_existing_match_snapshot() {
        let mut player = PlayerDisplayData {
            puuid: "player-1".into(),
            ..Default::default()
        };
        let existing_players_by_puuid = HashMap::from([(
            "player-1".to_string(),
            PlayerDisplayData {
                puuid: "player-1".into(),
                times_seen_before: 4,
                last_seen_at: "2026-03-08 12:00:00".into(),
                last_seen_game_name: "Existing".into(),
                last_seen_tag_line: "OLD".into(),
                ..Default::default()
            },
        )]);

        hydrate_player_history(
            &mut player,
            "local-player",
            &existing_players_by_puuid,
            Some(EncounterRecord {
                game_name: "Database".into(),
                tag_line: "NEW".into(),
                times_seen: 9,
                last_seen_at: "2026-03-10 12:00:00".into(),
            }),
        );

        assert_eq!(player.times_seen_before, 4);
        assert_eq!(player.last_seen_at, "2026-03-08 12:00:00");
        assert_eq!(player.last_seen_game_name, "Existing");
        assert_eq!(player.last_seen_tag_line, "OLD");
    }

    #[test]
    fn hydrate_player_history_uses_database_when_no_existing_snapshot() {
        let mut player = PlayerDisplayData {
            puuid: "player-1".into(),
            ..Default::default()
        };

        hydrate_player_history(
            &mut player,
            "local-player",
            &HashMap::new(),
            Some(EncounterRecord {
                game_name: "Database".into(),
                tag_line: "TAG".into(),
                times_seen: 3,
                last_seen_at: "2026-03-07 12:00:00".into(),
            }),
        );

        assert_eq!(player.times_seen_before, 3);
        assert_eq!(player.last_seen_at, "2026-03-07 12:00:00");
        assert_eq!(player.last_seen_game_name, "Database");
        assert_eq!(player.last_seen_tag_line, "TAG");
    }

    #[test]
    fn refreshes_encounter_identity_when_name_appears_mid_match() {
        let player = PlayerDisplayData {
            game_name: "Revealed".into(),
            tag_line: "TAG".into(),
            ..Default::default()
        };
        let existing_player = PlayerDisplayData::default();

        assert!(should_refresh_encounter_identity(
            &player,
            Some(&existing_player)
        ));
    }

    #[test]
    fn skips_encounter_identity_refresh_when_name_is_unchanged() {
        let player = PlayerDisplayData {
            game_name: "Same".into(),
            tag_line: "TAG".into(),
            ..Default::default()
        };
        let existing_player = PlayerDisplayData {
            game_name: "Same".into(),
            tag_line: "TAG".into(),
            ..Default::default()
        };

        assert!(!should_refresh_encounter_identity(
            &player,
            Some(&existing_player)
        ));
    }

    #[test]
    fn ignores_transient_waiting_for_client_while_in_match() {
        let previous_state = GameState::Ingame {
            match_id: "match-1".into(),
        };
        let mut waiting_polls = 0;

        for _ in 0..WAITING_FOR_CLIENT_DEBOUNCE_POLLS - 1 {
            assert_eq!(
                stabilize_game_state(
                    &previous_state,
                    GameState::WaitingForClient,
                    &mut waiting_polls,
                ),
                previous_state
            );
        }
    }

    #[test]
    fn accepts_waiting_for_client_after_threshold() {
        let previous_state = GameState::Ingame {
            match_id: "match-1".into(),
        };
        let mut waiting_polls = WAITING_FOR_CLIENT_DEBOUNCE_POLLS - 1;

        assert_eq!(
            stabilize_game_state(
                &previous_state,
                GameState::WaitingForClient,
                &mut waiting_polls,
            ),
            GameState::WaitingForClient
        );
    }

    #[test]
    fn resets_waiting_debounce_after_valid_state() {
        let previous_state = GameState::Ingame {
            match_id: "match-1".into(),
        };
        let mut waiting_polls = 2;

        assert_eq!(
            stabilize_game_state(&previous_state, GameState::Menu, &mut waiting_polls,),
            GameState::Menu
        );
        assert_eq!(waiting_polls, 0);
    }
}
