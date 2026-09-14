use crate::riot::api::RiotApiClient;
use anyhow::Result;

#[derive(Clone)]
pub struct MapInfo {
    pub name: String,
}

#[derive(Clone)]
pub struct MatchContext {
    pub map: MapInfo,
    pub queue: String,
    pub server_id: String,
}

pub async fn fetch_pregame_context(
    api: &mut RiotApiClient,
    match_id: &str,
) -> Result<MatchContext> {
    let pregame = api.get_pregame_match(match_id).await?;
    let map_id = pregame.map_i_d.unwrap_or_default();

    Ok(MatchContext {
        map: resolve_map_name(api, &map_id).await,
        queue: pregame.queue_i_d.unwrap_or_else(|| "competitive".into()),
        server_id: pregame.game_pod_id.unwrap_or_default(),
    })
}

pub async fn fetch_coregame_context(
    api: &mut RiotApiClient,
    match_id: &str,
) -> Result<MatchContext> {
    let coregame = api.get_coregame_match(match_id).await?;
    let map_id = coregame.map_i_d.unwrap_or_default();

    Ok(MatchContext {
        map: resolve_map_name(api, &map_id).await,
        queue: coregame.queue_i_d.unwrap_or_else(|| "competitive".into()),
        server_id: coregame.game_pod_id.unwrap_or_default(),
    })
}

async fn resolve_map_name(api: &mut RiotApiClient, map_id: &str) -> MapInfo {
    let name = api
        .resolve_map(map_id)
        .await
        .unwrap_or_else(|| fallback_map_name(map_id).to_string());

    MapInfo { name }
}

fn fallback_map_name(map_id: &str) -> &'static str {
    let map_id = map_id.to_ascii_lowercase();
    match map_id.as_str() {
        s if s.contains("ascent") => "Ascent",
        s if s.contains("bind") || s.contains("duality") => "Bind",
        s if s.contains("bonsai") => "Split",
        s if s.contains("triad") => "Haven",
        s if s.contains("port") => "Icebox",
        s if s.contains("foxtrot") => "Breeze",
        s if s.contains("canyon") => "Fracture",
        s if s.contains("pitt") => "Pearl",
        s if s.contains("jam") => "Lotus",
        s if s.contains("juliett") => "Sunset",
        s if s.contains("infinity") => "Abyss",
        s if s.contains("rook") => "Corrode",
        s if s.contains("plummet") => "Summit",
        s if s.contains("hurm") => "Team Deathmatch",
        _ => "Unknown",
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

#[cfg(test)]
mod tests {
    use super::fallback_map_name;

    #[test]
    fn fallback_resolves_recent_map_codenames_case_insensitively() {
        assert_eq!(fallback_map_name("/Game/Maps/Rook/Rook"), "Corrode");
        assert_eq!(fallback_map_name("/game/maps/plummet/plummet"), "Summit");
    }

    #[test]
    fn fallback_keeps_existing_map_codenames() {
        assert_eq!(fallback_map_name("/Game/Maps/Duality/Duality"), "Bind");
        assert_eq!(fallback_map_name("/Game/Maps/Infinity/Infinity"), "Abyss");
    }
}
