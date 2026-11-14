use super::activity::{build_activity, ActivityInput, ActivityView};
use crate::app::AppState;
use crate::game::state::GameState;
use discord_rich_presence::{
    activity::{Activity, Assets, Button, Party, Timestamps},
    DiscordIpc, DiscordIpcClient,
};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

const DISCORD_APP_ID: &str = "1571112498680176660";

/// Discord allows 5 activity updates per 20 seconds.
const UPDATE_INTERVAL: Duration = Duration::from_secs(5);
const RECONNECT_INTERVAL: Duration = Duration::from_secs(15);
/// Presence older than this means the data loop stopped (e.g. Valorant closed).
const PRESENCE_MAX_AGE: Duration = Duration::from_secs(30);
/// Discord IPC calls block; give up on one that hangs rather than stall.
const IPC_TIMEOUT: Duration = Duration::from_secs(3);

fn describe_error(payload: &Value) -> String {
    // Command errors nest code/message under "data"; handshake CLOSE frames
    // put them at the top level.
    let data = if payload["data"].is_object() {
        &payload["data"]
    } else {
        payload
    };
    match (data["code"].as_i64(), data["message"].as_str()) {
        (Some(code), Some(message)) => format!("{} (code {})", message, code),
        _ => payload.to_string(),
    }
}

/// Discriminant-only key so the elapsed timer persists within the same
/// variant but resets when the game state category changes.
fn state_discriminant(state: &GameState) -> u8 {
    match state {
        GameState::WaitingForClient => 0,
        GameState::Menu => 1,
        GameState::Pregame { .. } => 2,
        GameState::Ingame { .. } => 3,
    }
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[derive(Default)]
pub struct DiscordRpc {
    client: Option<DiscordIpcClient>,
}

impl DiscordRpc {
    pub fn is_connected(&self) -> bool {
        self.client.is_some()
    }

    pub fn connect(&mut self) {
        self.disconnect();

        let mut client = match DiscordIpcClient::new(DISCORD_APP_ID) {
            Ok(client) => client,
            Err(e) => {
                tracing::warn!("Failed to create Discord RPC client: {}", e);
                return;
            }
        };

        if let Err(e) = client.connect_ipc() {
            tracing::debug!("Discord RPC: Discord desktop app not reachable ({})", e);
            return;
        }

        // The crate's own handshake ignores Discord's reply, so a rejected
        // client ID would still look "connected". Do the handshake ourselves.
        let handshake = json!({ "v": 1, "client_id": DISCORD_APP_ID });
        let reply = client.send(handshake, 0).and_then(|_| client.recv());
        match reply {
            Ok((_, payload)) if payload["evt"] == "READY" => {
                let user = payload["data"]["user"]["username"]
                    .as_str()
                    .unwrap_or("unknown");
                self.client = Some(client);
                tracing::info!("Discord RPC connected (Discord user: {})", user);
            }
            Ok((_, payload)) => {
                tracing::warn!(
                    "Discord rejected the RPC handshake for application ID {}: {}",
                    DISCORD_APP_ID,
                    describe_error(&payload)
                );
                let _ = client.close();
            }
            Err(e) => {
                tracing::warn!("Discord RPC handshake failed: {}", e);
                let _ = client.close();
            }
        }
    }

    /// Sends the activity. Returns false if the connection was lost.
    pub fn set(&mut self, view: &ActivityView) -> bool {
        let Some(client) = &mut self.client else {
            return false;
        };

        let mut assets = Assets::new()
            .large_image(&view.large_image)
            .large_text(&view.large_text);
        if let Some(image) = &view.small_image {
            assets = assets.small_image(image);
            if let Some(text) = &view.small_text {
                assets = assets.small_text(text);
            }
        }

        let mut activity = Activity::new()
            .details(&view.details)
            .state(&view.state)
            .assets(assets);
        if let Some(start) = view.start_timestamp {
            activity = activity.timestamps(Timestamps::new().start(start));
        }
        if let Some((id, size, max)) = &view.party {
            activity = activity.party(Party::new().id(id).size([*size, *max]));
        }
        if let Some((label, url)) = &view.button {
            activity = activity.buttons(vec![Button::new(label, url)]);
        }

        // set_activity only writes; read Discord's reply so errors surface
        // and replies don't pile up unread in the pipe.
        match client.set_activity(activity).and_then(|_| client.recv()) {
            Ok((_, payload)) if payload["evt"] == "ERROR" => {
                tracing::warn!(
                    "Discord rejected the activity update: {}",
                    describe_error(&payload)
                );
                true
            }
            Ok(_) => true,
            Err(e) => {
                tracing::warn!("Discord RPC connection lost: {}", e);
                self.disconnect();
                false
            }
        }
    }

    pub fn clear(&mut self) -> bool {
        let Some(client) = &mut self.client else {
            return false;
        };
        match client.clear_activity().and_then(|_| client.recv()) {
            Ok(_) => true,
            Err(e) => {
                tracing::warn!("Discord RPC connection lost: {}", e);
                self.disconnect();
                false
            }
        }
    }

    pub fn disconnect(&mut self) {
        if let Some(mut client) = self.client.take() {
            let _ = client.close();
        }
    }
}

impl Drop for DiscordRpc {
    fn drop(&mut self) {
        self.disconnect();
    }
}

/// Runs a blocking IPC operation off the async runtime. On timeout the
/// client is abandoned (dropped when the call eventually returns).
async fn with_client<T: Send + 'static>(
    rpc: DiscordRpc,
    op: impl FnOnce(&mut DiscordRpc) -> T + Send + 'static,
) -> Option<(DiscordRpc, T)> {
    let task = tokio::task::spawn_blocking(move || {
        let mut rpc = rpc;
        let result = op(&mut rpc);
        (rpc, result)
    });
    match tokio::time::timeout(IPC_TIMEOUT, task).await {
        Ok(Ok(pair)) => Some(pair),
        Ok(Err(error)) => {
            tracing::warn!("Discord RPC task failed: {}", error);
            None
        }
        Err(_) => {
            tracing::warn!("Discord RPC call timed out; will reconnect");
            None
        }
    }
}

/// Keeps Discord Rich Presence in sync with `AppState` until quit.
pub async fn run(app_state: Arc<RwLock<AppState>>, quit_flag: Arc<AtomicBool>) {
    let mut rpc = DiscordRpc::default();
    let mut last_sent: Option<Option<ActivityView>> = None;
    let mut last_connect_attempt: Option<Instant> = None;
    let mut state_key: Option<u8> = None;
    let mut state_started_at = unix_now();
    let mut interval = tokio::time::interval(UPDATE_INTERVAL);

    loop {
        interval.tick().await;

        if quit_flag.load(Ordering::Relaxed) {
            if rpc.is_connected() {
                let _ = with_client(rpc, |rpc| rpc.clear()).await;
            }
            return;
        }

        let (view, enabled) = {
            let state = app_state.read().await;
            let enabled = state.config.behavior.discord_rpc;

            let key = state_discriminant(&state.game_state);
            if state_key != Some(key) {
                state_key = Some(key);
                state_started_at = unix_now();
            }

            let player = state
                .players
                .iter()
                .find(|p| !state.local_puuid.is_empty() && p.puuid == state.local_puuid);
            let presence = state.self_presence.as_ref().filter(|_| {
                state
                    .self_presence_updated
                    .is_some_and(|at| at.elapsed() < PRESENCE_MAX_AGE)
            });
            let view = build_activity(&ActivityInput {
                game_state: &state.game_state,
                context: state.match_context.as_ref(),
                presence,
                player,
                config: &state.config.discord,
                state_started_at,
            });
            (view, enabled)
        };

        if !enabled {
            if rpc.is_connected() {
                tracing::info!("Discord RPC disabled; clearing activity");
                if let Some((mut returned, _)) = with_client(rpc, |rpc| rpc.clear()).await {
                    returned.disconnect();
                }
                rpc = DiscordRpc::default();
            }
            last_sent = None;
            continue;
        }

        if !rpc.is_connected() {
            let due = last_connect_attempt.is_none_or(|at| at.elapsed() >= RECONNECT_INTERVAL);
            if !due {
                continue;
            }
            last_connect_attempt = Some(Instant::now());
            rpc = match with_client(rpc, |rpc| rpc.connect()).await {
                Some((connected, ())) => connected,
                None => DiscordRpc::default(),
            };
            if !rpc.is_connected() {
                continue;
            }
            last_sent = None;
        }

        if last_sent.as_ref() == Some(&view) {
            continue;
        }

        let to_send = view.clone();
        rpc = match with_client(rpc, move |rpc| match &to_send {
            Some(view) => rpc.set(view),
            None => rpc.clear(),
        })
        .await
        {
            Some((returned, true)) => {
                last_sent = Some(view);
                returned
            }
            Some((returned, false)) => returned,
            None => DiscordRpc::default(),
        };
    }
}
