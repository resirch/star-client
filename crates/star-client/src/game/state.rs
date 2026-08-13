use crate::riot::api::RiotApiClient;
use crate::riot::types::{PlayerSessionResponse, PrivatePresence};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectionSource {
    PregamePlayer,
    CoregamePlayer,
    PlayerSession,
    Presence,
    NoSelfPresence,
    PresenceUnavailable,
}

impl std::fmt::Display for DetectionSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            DetectionSource::PregamePlayer => "pregame-player",
            DetectionSource::CoregamePlayer => "coregame-player",
            DetectionSource::PlayerSession => "player-session",
            DetectionSource::Presence => "presence",
            DetectionSource::NoSelfPresence => "no-self-presence",
            DetectionSource::PresenceUnavailable => "presence-unavailable",
        };
        write!(f, "{name}")
    }
}

#[derive(Debug, Clone)]
pub struct GameStateDetection {
    pub state: GameState,
    pub source: DetectionSource,
    pub evidence: String,
    pub auth_refresh_recommended: bool,
}

pub async fn detect_game_state(api: &RiotApiClient) -> Result<GameStateDetection> {
    // These endpoints are independent. Polling them together prevents three
    // request timeouts from serially delaying the data loop.
    let (pregame_result, coregame_result, session_result) = tokio::join!(
        api.get_pregame_player(),
        api.get_coregame_player(),
        api.get_player_session()
    );

    let mut pregame_evidence = match &pregame_result {
        Ok(pregame) => match_id_evidence(&pregame.match_i_d),
        Err(error) => format!("error={}", compact_error(error)),
    };
    let mut coregame_evidence = match &coregame_result {
        Ok(coregame) => match_id_evidence(&coregame.match_i_d),
        Err(error) => format!("error={}", compact_error(error)),
    };
    let session_evidence = match &session_result {
        Ok(session) => format!(
            "loop={}, match_id={}",
            display_value(&session.loop_state),
            display_value(&session.loop_state_metadata)
        ),
        Err(error) => format!("error={}", compact_error(error)),
    };
    let glz_auth_failure = pregame_result.as_ref().err().is_some_and(is_auth_error)
        || coregame_result.as_ref().err().is_some_and(is_auth_error)
        || session_result.as_ref().err().is_some_and(is_auth_error);
    let session_unavailable = match &session_result {
        Ok(session) => session.loop_state.trim().is_empty(),
        Err(_) => true,
    };

    // Player session describes the client's current loop and should win over
    // a player endpoint that may still return the just-finished match.
    if let Ok(session) = &session_result {
        if let Some(session_state) = state_from_player_session(session) {
            if session_state.is_in_match() {
                return Ok(detection(
                    session_state,
                    DetectionSource::PlayerSession,
                    &pregame_evidence,
                    &coregame_evidence,
                    &session_evidence,
                    "not-checked",
                ));
            }
        }
    }

    if let Ok(pregame) = &pregame_result {
        if let Some(match_id) = non_empty_match_id(&pregame.match_i_d) {
            return Ok(detection(
                GameState::Pregame { match_id },
                DetectionSource::PregamePlayer,
                &pregame_evidence,
                &coregame_evidence,
                &session_evidence,
                "not-checked",
            ));
        }
    }

    if let Ok(coregame) = &coregame_result {
        if let Some(match_id) = non_empty_match_id(&coregame.match_i_d) {
            return Ok(detection(
                GameState::Ingame { match_id },
                DetectionSource::CoregamePlayer,
                &pregame_evidence,
                &coregame_evidence,
                &session_evidence,
                "not-checked",
            ));
        }
    }

    let mut session_has_active_loop_without_match = false;
    let mut retried_pregame = false;
    let mut retried_coregame = false;

    if let Ok(session) = &session_result {
        match session.loop_state.to_ascii_lowercase().as_str() {
            "pregame" => {
                session_has_active_loop_without_match = true;
                retried_pregame = true;
                match api.get_pregame_player().await {
                    Ok(pregame) => {
                        pregame_evidence = format!(
                            "{}; forced_retry={}",
                            pregame_evidence,
                            match_id_evidence(&pregame.match_i_d)
                        );
                        if let Some(match_id) = non_empty_match_id(&pregame.match_i_d) {
                            return Ok(detection(
                                GameState::Pregame { match_id },
                                DetectionSource::PregamePlayer,
                                &pregame_evidence,
                                &coregame_evidence,
                                &session_evidence,
                                "not-checked",
                            ));
                        }
                    }
                    Err(error) => {
                        pregame_evidence = format!(
                            "{}; forced_retry_error={}",
                            pregame_evidence,
                            compact_error(&error)
                        );
                    }
                }
            }
            "ingame" => {
                session_has_active_loop_without_match = true;
                retried_coregame = true;
                match api.get_coregame_player().await {
                    Ok(coregame) => {
                        coregame_evidence = format!(
                            "{}; forced_retry={}",
                            coregame_evidence,
                            match_id_evidence(&coregame.match_i_d)
                        );
                        if let Some(match_id) = non_empty_match_id(&coregame.match_i_d) {
                            return Ok(detection(
                                GameState::Ingame { match_id },
                                DetectionSource::CoregamePlayer,
                                &pregame_evidence,
                                &coregame_evidence,
                                &session_evidence,
                                "not-checked",
                            ));
                        }
                    }
                    Err(error) => {
                        coregame_evidence = format!(
                            "{}; forced_retry_error={}",
                            coregame_evidence,
                            compact_error(&error)
                        );
                    }
                }
            }
            _ => {}
        }
    }

    // Presence is the final independent signal. If it says the client has
    // entered a match but lacks an ID, force one fresh player-endpoint read.
    match api.get_self_presence().await {
        Ok(Some(presence)) => {
            let presence_loop = presence.session_loop_state.to_ascii_lowercase();
            let presence_evidence = format!(
                "valid={}, loop={}, match_id={}, provisioning={}, queue={}",
                presence.is_valid,
                display_value(&presence.session_loop_state),
                display_value(&presence.match_id),
                display_value(&presence.provisioning_flow),
                display_value(&presence.queue_id)
            );

            let pregame_match_id = if presence_loop == "pregame" && !retried_pregame {
                match api.get_pregame_player().await {
                    Ok(pregame) => {
                        pregame_evidence = format!(
                            "{}; forced_retry={}",
                            pregame_evidence,
                            match_id_evidence(&pregame.match_i_d)
                        );
                        Some(pregame.match_i_d)
                    }
                    Err(error) => {
                        pregame_evidence = format!(
                            "{}; forced_retry_error={}",
                            pregame_evidence,
                            compact_error(&error)
                        );
                        None
                    }
                }
            } else {
                None
            };
            let coregame_match_id = if presence_loop == "ingame" && !retried_coregame {
                match api.get_coregame_player().await {
                    Ok(coregame) => {
                        coregame_evidence = format!(
                            "{}; forced_retry={}",
                            coregame_evidence,
                            match_id_evidence(&coregame.match_i_d)
                        );
                        Some(coregame.match_i_d)
                    }
                    Err(error) => {
                        coregame_evidence = format!(
                            "{}; forced_retry_error={}",
                            coregame_evidence,
                            compact_error(&error)
                        );
                        None
                    }
                }
            } else {
                None
            };

            let presence_state =
                state_from_presence(&presence, pregame_match_id, coregame_match_id);
            let (state, source) = if session_has_active_loop_without_match
                && matches!(presence_state, GameState::Menu)
            {
                (GameState::WaitingForClient, DetectionSource::PlayerSession)
            } else {
                (presence_state, DetectionSource::Presence)
            };

            let mut detection = detection(
                state,
                source,
                &pregame_evidence,
                &coregame_evidence,
                &session_evidence,
                &presence_evidence,
            );
            detection.auth_refresh_recommended =
                should_refresh_auth_for_presence(session_unavailable, glz_auth_failure, &presence);
            Ok(detection)
        }
        Ok(None) => {
            let state = if session_has_active_loop_without_match {
                GameState::WaitingForClient
            } else {
                GameState::Menu
            };
            let mut detection = detection(
                state,
                if session_has_active_loop_without_match {
                    DetectionSource::PlayerSession
                } else {
                    DetectionSource::NoSelfPresence
                },
                &pregame_evidence,
                &coregame_evidence,
                &session_evidence,
                "self=missing",
            );
            detection.auth_refresh_recommended = glz_auth_failure;
            Ok(detection)
        }
        Err(error) => Ok(detection(
            GameState::WaitingForClient,
            DetectionSource::PresenceUnavailable,
            &pregame_evidence,
            &coregame_evidence,
            &session_evidence,
            &format!("error={}", compact_error(&error)),
        )),
    }
}

fn detection(
    state: GameState,
    source: DetectionSource,
    pregame: &str,
    coregame: &str,
    session: &str,
    presence: &str,
) -> GameStateDetection {
    GameStateDetection {
        state,
        source,
        evidence: format!(
            "pregame=[{}], coregame=[{}], session=[{}], presence=[{}]",
            pregame, coregame, session, presence
        ),
        auth_refresh_recommended: false,
    }
}

fn match_id_evidence(match_id: &str) -> String {
    format!("match_id={}", display_value(match_id))
}

fn display_value(value: &str) -> &str {
    let value = value.trim();
    if value.is_empty() {
        "<empty>"
    } else {
        value
    }
}

fn compact_error(error: &anyhow::Error) -> String {
    let text = error.to_string().replace(['\r', '\n'], " ");
    text.chars().take(240).collect()
}

fn is_auth_error(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<reqwest::Error>()
        .and_then(reqwest::Error::status)
        .is_some_and(|status| {
            status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN
        })
}

fn should_refresh_auth_for_presence(
    session_unavailable: bool,
    glz_auth_failure: bool,
    presence: &PrivatePresence,
) -> bool {
    glz_auth_failure
        || (session_unavailable
            && presence
                .provisioning_flow
                .eq_ignore_ascii_case("matchmaking"))
}

fn state_from_player_session(session: &PlayerSessionResponse) -> Option<GameState> {
    match session.loop_state.to_ascii_lowercase().as_str() {
        "pregame" => Some(
            non_empty_match_id(&session.loop_state_metadata)
                .map(|match_id| GameState::Pregame { match_id })
                .unwrap_or(GameState::WaitingForClient),
        ),
        "ingame" => Some(
            non_empty_match_id(&session.loop_state_metadata)
                .map(|match_id| GameState::Ingame { match_id })
                .unwrap_or(GameState::WaitingForClient),
        ),
        _ => None,
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
                GameState::WaitingForClient
            } else {
                GameState::Pregame { match_id }
            }
        }
        "ingame" => {
            let match_id = coregame_match_id
                .or_else(|| non_empty_match_id(&presence.match_id))
                .unwrap_or_default();
            if match_id.is_empty() {
                GameState::WaitingForClient
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
    use super::{should_refresh_auth_for_presence, state_from_player_session, state_from_presence};
    use crate::game::state::GameState;
    use crate::riot::types::{PlayerSessionResponse, PrivatePresence};

    #[test]
    fn player_session_detects_ingame_when_presence_can_be_stale() {
        let session = PlayerSessionResponse {
            loop_state: "INGAME".into(),
            loop_state_metadata: "new-match".into(),
        };

        assert_eq!(
            state_from_player_session(&session),
            Some(GameState::Ingame {
                match_id: "new-match".into()
            })
        );
    }

    #[test]
    fn active_player_session_without_match_id_waits_for_forced_refetch() {
        let session = PlayerSessionResponse {
            loop_state: "PREGAME".into(),
            loop_state_metadata: String::new(),
        };

        assert_eq!(
            state_from_player_session(&session),
            Some(GameState::WaitingForClient)
        );
    }

    #[test]
    fn menu_player_session_defers_to_presence() {
        let session = PlayerSessionResponse {
            loop_state: "MENUS".into(),
            loop_state_metadata: String::new(),
        };

        assert_eq!(state_from_player_session(&session), None);
    }

    #[test]
    fn unavailable_session_during_matchmaking_requests_auth_refresh() {
        let matchmaking = PrivatePresence {
            provisioning_flow: "Matchmaking".into(),
            ..Default::default()
        };
        let ordinary_menu = PrivatePresence {
            provisioning_flow: "Invalid".into(),
            ..Default::default()
        };

        assert!(should_refresh_auth_for_presence(true, false, &matchmaking));
        assert!(!should_refresh_auth_for_presence(
            true,
            false,
            &ordinary_menu
        ));
        assert!(should_refresh_auth_for_presence(
            false,
            true,
            &ordinary_menu
        ));
    }

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
    fn ingame_presence_without_any_match_id_waits_for_match_id() {
        let presence = PrivatePresence {
            session_loop_state: "ingame".into(),
            ..Default::default()
        };

        assert_eq!(
            state_from_presence(&presence, None, None),
            GameState::WaitingForClient
        );
    }

    #[test]
    fn pregame_presence_without_any_match_id_waits_for_match_id() {
        let presence = PrivatePresence {
            session_loop_state: "pregame".into(),
            ..Default::default()
        };

        assert_eq!(
            state_from_presence(&presence, None, None),
            GameState::WaitingForClient
        );
    }
}
