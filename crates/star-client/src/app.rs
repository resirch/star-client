use crate::config::Config;
use crate::discord::rpc::DiscordRpc;
use crate::game::history::PlayerHistory;
use crate::game::match_data::{self, MatchContext};
use crate::game::party;
use crate::game::players;
use crate::game::state::{self, GameState};
use crate::riot::api::RiotApiClient;
use crate::riot::types::PlayerDisplayData;
use crate::star::client::StarClient;
use crate::star::presence;
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
}

impl AppState {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            game_state: GameState::WaitingForClient,
            players: Vec::new(),
            match_context: None,
            auto_visible: false,
            last_match_id: String::new(),
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
    {
        let state = app_state.read().await;
        if state.config.behavior.discord_rpc {
            discord.connect();
        }
    }

    let mut poll_interval = tokio::time::interval(std::time::Duration::from_secs(5));

    loop {
        poll_interval.tick().await;

        if quit_flag.load(Ordering::Relaxed) {
            discord.disconnect();
            break;
        }

        let mut api_guard = api.write().await;
        let new_state =
            state::detect_game_state(&api_guard).await.unwrap_or(GameState::Menu);

        let config = {
            let state = app_state.read().await;
            state.config.clone()
        };

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
                GameState::Menu => {
                    state.auto_visible = false;
                    state.players.clear();
                    state.match_context = None;
                }
                _ => {}
            }
        }

        let is_new_match = {
            let state = app_state.read().await;
            !match_id.is_empty() && match_id != state.last_match_id
        };

        if is_new_match || (state_changed && !match_id.is_empty()) {
            // Phase 1: Fetch basic player info (names, agents, levels, teams)
            tracing::debug!(
                "Player retrieval triggered: state_changed={}, is_new_match={}, game_state={:?}, match_id={}",
                state_changed,
                is_new_match,
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

            if let Some(history) = &history {
                for p in &players_data {
                    if !p.game_name.is_empty() {
                        let _ =
                            history.record_encounter(&p.puuid, &p.game_name, &p.tag_line);
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
                players::enrich_player(&api_guard, &mut player, &current_season).await;

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
                tracing::debug!(
                    "Re-enriching {} incomplete players",
                    unenriched.len()
                );
                let current_season =
                    api_guard.get_current_season_id().await.ok().flatten();

                for (idx, puuid) in &unenriched {
                    let mut player = {
                        let state = app_state.read().await;
                        if *idx >= state.players.len() {
                            continue;
                        }
                        state.players[*idx].clone()
                    };

                    players::enrich_player(&api_guard, &mut player, &current_season)
                        .await;

                    let mut state = app_state.write().await;
                    if *idx < state.players.len()
                        && state.players[*idx].puuid == *puuid
                    {
                        state.players[*idx] = player;
                    }
                }
            }
        }

        if config.behavior.discord_rpc {
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
