use crate::config::Config;
use crate::discord::rpc::DiscordRpc;
use crate::game::history::PlayerHistory;
use crate::game::match_data::{self, MatchContext};
use crate::game::party;
use crate::game::players;
use crate::game::state::{self, GameState};
use crate::overlay::ui::OverlayUi;
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
    pub overlay: OverlayUi,
    pub last_match_id: String,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            game_state: GameState::WaitingForClient,
            players: Vec::new(),
            match_context: None,
            overlay: OverlayUi::new(),
            last_match_id: String::new(),
        }
    }
}

pub async fn run_data_loop(
    app_state: Arc<RwLock<AppState>>,
    api: Arc<RwLock<RiotApiClient>>,
    star_client: Arc<StarClient>,
    hotkey_toggle: Arc<AtomicBool>,
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

        // Check hotkey toggle
        if hotkey_toggle.swap(false, Ordering::Relaxed) {
            let mut state = app_state.write().await;
            state.overlay.visible = !state.overlay.visible;
            tracing::debug!("Overlay toggled: {}", state.overlay.visible);
        }

        let mut api_guard = api.write().await;
        let new_state = state::detect_game_state(&api_guard).await.unwrap_or(GameState::Menu);

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

        // Auto-show/hide overlay on state transitions
        if state_changed {
            let mut state = app_state.write().await;
            state.game_state = new_state.clone();

            match &new_state {
                GameState::Pregame { .. } if config.behavior.auto_show_pregame => {
                    state.overlay.visible = true;
                }
                GameState::Ingame { .. } if config.behavior.auto_hide_ingame => {
                    state.overlay.visible = false;
                }
                GameState::Menu => {
                    state.overlay.visible = false;
                    state.players.clear();
                    state.match_context = None;
                }
                _ => {}
            }
        }

        // Fetch player data when in a match
        let should_fetch = {
            let state = app_state.read().await;
            !match_id.is_empty() && match_id != state.last_match_id
        };

        if should_fetch || (state_changed && !match_id.is_empty()) {
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

            // Party detection
            if config.behavior.party_finder && !players_data.is_empty() {
                party::detect_parties(&api_guard, &mut players_data).await;
            }

            // Star presence
            if config.star.enabled {
                presence::mark_star_users(&star_client, &mut players_data).await;
            }

            // Record encounters
            if let Some(history) = &history {
                for p in &players_data {
                    if !p.game_name.is_empty() {
                        let _ = history.record_encounter(&p.puuid, &p.game_name, &p.tag_line);
                    }
                }
            }

            // Fetch match context
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

            let mut state = app_state.write().await;
            state.players = players_data;
            state.match_context = ctx;
            state.last_match_id = match_id.clone();
        }

        // Update Discord RPC
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
