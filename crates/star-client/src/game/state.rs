use crate::riot::api::RiotApiClient;
use crate::riot::types::PrivatePresence;
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
            let pregame_match_id = if presence.session_loop_state.eq_ignore_ascii_case("pregame") {
                api.get_pregame_player()
                    .await
                    .ok()
                    .map(|pregame| pregame.match_i_d)
            } else {
                None
            };
            let coregame_match_id = if presence.session_loop_state.eq_ignore_ascii_case("ingame") {
                api.get_coregame_player()
                    .await
                    .ok()
                    .map(|coregame| coregame.match_i_d)
            } else {
                None
            };

            Ok(state_from_presence(
                &presence,
                pregame_match_id,
                coregame_match_id,
            ))
        }
        Ok(None) => Ok(GameState::Menu),
        Err(_) => Ok(GameState::WaitingForClient),
    }
}

fn state_from_presence(
    presence: &PrivatePresence,
    pregame_match_id: Option<String>,
    coregame_match_id: Option<String>,
) -> GameState {
    match presence.session_loop_state.to_ascii_lowercase().as_str() {
        "pregame" => {
            let match_id = pregame_match_id
                .or_else(|| non_empty_match_id(&presence.match_id))
                .unwrap_or_default();
            if match_id.is_empty() {
                GameState::Menu
            } else {
                GameState::Pregame { match_id }
            }
        }
        "ingame" => {
            let match_id = coregame_match_id
                .or_else(|| non_empty_match_id(&presence.match_id))
                .unwrap_or_default();
            if match_id.is_empty() {
                GameState::Menu
            } else {
                GameState::Ingame { match_id }
            }
        }
        _ => GameState::Menu,
    }
}

fn non_empty_match_id(match_id: &str) -> Option<String> {
    let trimmed = match_id.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::state_from_presence;
    use crate::game::state::GameState;
    use crate::riot::types::PrivatePresence;

    #[test]
    fn presence_keeps_ingame_state_when_coregame_player_lookup_fails() {
        let presence = PrivatePresence {
            session_loop_state: "INGAME".into(),
            match_id: "match-2".into(),
            ..Default::default()
        };

        assert_eq!(
            state_from_presence(&presence, None, None),
            GameState::Ingame {
                match_id: "match-2".into()
            }
        );
    }

    #[test]
    fn presence_prefers_direct_match_id_when_retry_succeeds() {
        let presence = PrivatePresence {
            session_loop_state: "pregame".into(),
            match_id: "stale-id".into(),
            ..Default::default()
        };

        assert_eq!(
            state_from_presence(&presence, Some("fresh-id".into()), None),
            GameState::Pregame {
                match_id: "fresh-id".into()
            }
        );
    }

    #[test]
    fn ingame_presence_without_any_match_id_falls_back_to_menu() {
        let presence = PrivatePresence {
            session_loop_state: "ingame".into(),
            ..Default::default()
        };

        assert_eq!(state_from_presence(&presence, None, None), GameState::Menu);
    }
}
