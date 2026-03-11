use crate::riot::api::RiotApiClient;
use anyhow::Result;

pub struct MapInfo {
    pub name: String,
    pub id: String,
}

pub struct MatchContext {
    pub map: MapInfo,
    pub mode: String,
    pub queue: String,
    pub server_id: String,
}

pub async fn fetch_pregame_context(api: &RiotApiClient, match_id: &str) -> Result<MatchContext> {
    let pregame = api.get_pregame_match(match_id).await?;
    let map_id = pregame.map_i_d.unwrap_or_default();

    Ok(MatchContext {
        map: resolve_map_name(&map_id),
        mode: pregame.mode.unwrap_or_else(|| "Unknown".into()),
        queue: pregame.queue_i_d.unwrap_or_else(|| "competitive".into()),
        server_id: pregame.game_pod_id.unwrap_or_default(),
    })
}

pub async fn fetch_coregame_context(api: &RiotApiClient, match_id: &str) -> Result<MatchContext> {
    let coregame = api.get_coregame_match(match_id).await?;
    let map_id = coregame.map_i_d.unwrap_or_default();

    Ok(MatchContext {
        map: resolve_map_name(&map_id),
        mode: coregame.mode_i_d.unwrap_or_else(|| "Unknown".into()),
        queue: coregame.queue_i_d.unwrap_or_else(|| "competitive".into()),
        server_id: coregame.game_pod_id.unwrap_or_default(),
    })
}

fn resolve_map_name(map_url: &str) -> MapInfo {
    let name = match map_url {
        s if s.contains("Ascent") => "Ascent",
        s if s.contains("Bind") || s.contains("Duality") => "Bind",
        s if s.contains("Bonsai") => "Split",
        s if s.contains("Triad") => "Haven",
        s if s.contains("Port") => "Icebox",
        s if s.contains("Foxtrot") => "Breeze",
        s if s.contains("Canyon") => "Fracture",
        s if s.contains("Pitt") => "Pearl",
        s if s.contains("Jam") => "Lotus",
        s if s.contains("Juliett") => "Sunset",
        s if s.contains("HURM") => "Team Deathmatch",
        s if s.contains("Infinity") => "Abyss",
        _ => "Unknown",
    };

    MapInfo {
        name: name.to_string(),
        id: map_url.to_string(),
    }
}

pub fn mode_display_name(mode_id: &str) -> &str {
    match mode_id {
        s if s.contains("competitive") => "Competitive",
        s if s.contains("unrated") => "Unrated",
        s if s.contains("spikerush") => "Spike Rush",
        s if s.contains("deathmatch") => "Deathmatch",
        s if s.contains("ggteam") => "Escalation",
        s if s.contains("swiftplay") => "Swiftplay",
        s if s.contains("hurm") => "Team Deathmatch",
        s if s.contains("premier") => "Premier",
        _ => "Unknown",
    }
}
