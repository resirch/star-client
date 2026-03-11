use super::types::RiotAuth;

pub fn pd_base(auth: &RiotAuth) -> String {
    format!("https://pd.{}.a.pvp.net", auth.shard)
}

pub fn glz_base(auth: &RiotAuth) -> String {
    format!("https://glz-{}-1.{}.a.pvp.net", auth.region, auth.shard)
}

pub fn shared_base(auth: &RiotAuth) -> String {
    format!("https://shared.{}.a.pvp.net", auth.shard)
}

pub fn mmr(auth: &RiotAuth, puuid: &str) -> String {
    format!("{}/mmr/v1/players/{}", pd_base(auth), puuid)
}

pub fn competitive_updates(auth: &RiotAuth, puuid: &str) -> String {
    format!(
        "{}/mmr/v1/players/{}/competitiveupdates?startIndex=0&endIndex=10",
        pd_base(auth),
        puuid
    )
}

pub fn match_details(auth: &RiotAuth, match_id: &str) -> String {
    format!("{}/match-details/v1/matches/{}", pd_base(auth), match_id)
}

pub fn match_history(auth: &RiotAuth, puuid: &str) -> String {
    format!(
        "{}/match-history/v1/history/{}?startIndex=0&endIndex=5",
        pd_base(auth),
        puuid
    )
}

pub fn name_service(auth: &RiotAuth) -> String {
    format!("{}/name-service/v2/players", pd_base(auth))
}

pub fn pregame_player(auth: &RiotAuth, puuid: &str) -> String {
    format!("{}/pregame/v1/players/{}", glz_base(auth), puuid)
}

pub fn pregame_match(auth: &RiotAuth, match_id: &str) -> String {
    format!("{}/pregame/v1/matches/{}", glz_base(auth), match_id)
}

pub fn coregame_player(auth: &RiotAuth, puuid: &str) -> String {
    format!("{}/core-game/v1/players/{}", glz_base(auth), puuid)
}

pub fn coregame_match(auth: &RiotAuth, match_id: &str) -> String {
    format!("{}/core-game/v1/matches/{}", glz_base(auth), match_id)
}

pub fn coregame_loadouts(auth: &RiotAuth, match_id: &str) -> String {
    format!(
        "{}/core-game/v1/matches/{}/loadouts",
        glz_base(auth),
        match_id
    )
}

pub fn content(auth: &RiotAuth) -> String {
    format!("{}/content-service/v3/content", shared_base(auth))
}

pub fn local_presence(port: u16) -> String {
    format!("https://127.0.0.1:{}/chat/v4/presences", port)
}

pub fn local_websocket(port: u16, password: &str) -> String {
    let basic = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        format!("riot:{}", password),
    );
    format!("wss://127.0.0.1:{}/?authorization=Basic {}", port, basic)
}
