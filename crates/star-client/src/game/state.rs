use crate::riot::api::RiotApiClient;
use anyhow::Result;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameState {
    WaitingForClient,
    Menu,
    Pregame { match_id: String },
    Ingame { match_id: String },
}

impl std::fmt::Display for GameState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GameState::WaitingForClient => write!(f, "Waiting for Client"),
            GameState::Menu => write!(f, "Menu"),
            GameState::Pregame { .. } => write!(f, "Agent Select"),
            GameState::Ingame { .. } => write!(f, "In Game"),
        }
    }
}

impl GameState {
    pub fn is_in_match(&self) -> bool {
        matches!(self, GameState::Pregame { .. } | GameState::Ingame { .. })
    }
}

pub async fn detect_game_state(api: &RiotApiClient) -> Result<GameState> {
    // Try pregame first (agent select)
    if let Ok(pregame) = api.get_pregame_player().await {
        return Ok(GameState::Pregame {
            match_id: pregame.match_i_d,
        });
    }

    // Try coregame (in match)
    if let Ok(coregame) = api.get_coregame_player().await {
        return Ok(GameState::Ingame {
            match_id: coregame.match_i_d,
        });
    }

    // Check presence to see if client is running
    match api.get_self_presence().await {
        Ok(Some(presence)) => {
            let state = presence.session_loop_state.to_lowercase();
            match state.as_str() {
                "pregame" => {
                    if let Ok(pregame) = api.get_pregame_player().await {
                        Ok(GameState::Pregame {
                            match_id: pregame.match_i_d,
                        })
                    } else {
                        Ok(GameState::Menu)
                    }
                }
                "ingame" => {
                    if let Ok(coregame) = api.get_coregame_player().await {
                        Ok(GameState::Ingame {
                            match_id: coregame.match_i_d,
                        })
                    } else {
                        Ok(GameState::Menu)
                    }
                }
                _ => Ok(GameState::Menu),
            }
        }
        Ok(None) => Ok(GameState::Menu),
        Err(_) => Ok(GameState::WaitingForClient),
    }
}
