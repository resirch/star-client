use super::client::StarClient;
use crate::riot::types::PlayerDisplayData;
use std::sync::Arc;

/// Marks players who are also star-client users.
pub async fn mark_star_users(star_client: &Arc<StarClient>, players: &mut [PlayerDisplayData]) {
    let puuids: Vec<String> = players.iter().map(|p| p.puuid.clone()).collect();

    match star_client.query(&puuids).await {
        Ok(star_users) => {
            for player in players.iter_mut() {
                player.is_star_user = star_users.contains(&player.puuid);
            }
        }
        Err(e) => {
            tracing::warn!("Star query failed: {}", e);
        }
    }
}
