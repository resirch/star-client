use crate::riot::types::MatchDetailsResponse;

pub struct PerformanceStats {
    pub kd: f64,
    pub kills: i32,
    pub deaths: i32,
}

pub fn extract_player_performance(
    match_details: &MatchDetailsResponse,
    puuid: &str,
) -> Option<PerformanceStats> {
    for player in &match_details.players {
        if player.subject == puuid {
            if let Some(stats) = &player.stats {
                let kills = stats.kills.unwrap_or(0);
                let deaths = stats.deaths.unwrap_or(1).max(1);
                let kd = (kills as f64 / deaths as f64 * 100.0).round() / 100.0;

                return Some(PerformanceStats { kd, kills, deaths });
            }
        }
    }
    None
}
