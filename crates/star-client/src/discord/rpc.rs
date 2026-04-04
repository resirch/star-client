use crate::game::match_data::{mode_display_name, MatchContext};
use crate::game::state::GameState;
use discord_rich_presence::{
    activity::{Activity, Assets, Timestamps},
    DiscordIpc, DiscordIpcClient,
};
use std::time::{SystemTime, UNIX_EPOCH};

const DISCORD_APP_ID: &str = "1200000000000000000";

/// Discriminant-only key so timestamp persists within the same variant
/// but resets when the game state category changes (e.g. Menu → Pregame).
fn state_discriminant(state: &GameState) -> u8 {
    match state {
        GameState::WaitingForClient => 0,
        GameState::Menu => 1,
        GameState::Pregame { .. } => 2,
        GameState::Ingame { .. } => 3,
    }
}

pub struct DiscordRpc {
    client: Option<DiscordIpcClient>,
    connected: bool,
    last_state_discriminant: Option<u8>,
    state_start_time: i64,
}

impl DiscordRpc {
    pub fn new() -> Self {
        Self {
            client: None,
            connected: false,
            last_state_discriminant: None,
            state_start_time: 0,
        }
    }

    pub fn connect(&mut self) {
        match DiscordIpcClient::new(DISCORD_APP_ID) {
            Ok(mut client) => {
                if client.connect().is_ok() {
                    self.client = Some(client);
                    self.connected = true;
                    tracing::info!("Discord RPC connected");
                } else {
                    tracing::warn!("Failed to connect Discord RPC");
                }
            }
            Err(e) => {
                tracing::warn!("Failed to create Discord RPC client: {}", e);
            }
        }
    }

    pub fn update(
        &mut self,
        state: &GameState,
        context: Option<&MatchContext>,
        rank_name: &str,
        agent_name: &str,
    ) {
        if !self.connected {
            return;
        }

        let client = match &mut self.client {
            Some(c) => c,
            None => return,
        };

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let disc = state_discriminant(state);
        if self.last_state_discriminant != Some(disc) {
            self.last_state_discriminant = Some(disc);
            self.state_start_time = now;
        }

        let (details, state_text) = match state {
            GameState::Menu => ("In Menus".to_string(), rank_name.to_string()),
            GameState::Pregame { .. } => {
                let map = context.map(|c| c.map.name.as_str()).unwrap_or("Unknown");
                let mode = context
                    .map(|c| mode_display_name(&c.queue))
                    .unwrap_or("Unknown");
                (
                    format!("Agent Select - {}", map),
                    format!("{} | {}", mode, rank_name),
                )
            }
            GameState::Ingame { .. } => {
                let map = context.map(|c| c.map.name.as_str()).unwrap_or("Unknown");
                let mode = context
                    .map(|c| mode_display_name(&c.queue))
                    .unwrap_or("Unknown");
                (
                    format!("In Game - {}", map),
                    format!("{} | {} | {}", mode, rank_name, agent_name),
                )
            }
            GameState::WaitingForClient => ("Star Client".to_string(), "Waiting...".to_string()),
        };

        let activity = Activity::new()
            .details(&details)
            .state(&state_text)
            .timestamps(Timestamps::new().start(self.state_start_time))
            .assets(Assets::new().large_text("Star Client"));

        let _ = client.set_activity(activity);
    }

    pub fn clear(&mut self) {
        if let Some(client) = &mut self.client {
            let _ = client.clear_activity();
        }
    }

    pub fn disconnect(&mut self) {
        if let Some(client) = &mut self.client {
            let _ = client.close();
        }
        self.connected = false;
    }
}

impl Drop for DiscordRpc {
    fn drop(&mut self) {
        self.disconnect();
    }
}
