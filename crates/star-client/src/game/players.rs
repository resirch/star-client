use crate::config::Config;
use crate::riot::api::RiotApiClient;
use crate::riot::types::*;
use crate::stats::performance::extract_player_performance;
use anyhow::Result;
use std::collections::HashMap;

pub const OVERLAY_WEAPONS: &[&str] = &[
    "Vandal", "Phantom", "Operator", "Sheriff", "Spectre", "Classic",
];

#[derive(Debug, Default)]
struct EquippedSkin {
    name: String,
    level: usize,
    total_levels: usize,
    color: egui::Color32,
}

#[derive(Debug, Clone, Default)]
pub struct SeasonLookup {
    act_order: Vec<String>,
}

pub fn build_season_lookup(content: &ContentResponse) -> SeasonLookup {
    let mut lookup = SeasonLookup::default();
    let mut active_act_index = None;

    if let Some(seasons) = &content.seasons {
        for season in seasons {
            if season.season_type.as_deref() != Some("act") {
                continue;
            }

            let Some(season_id) = season.i_d.clone() else {
                continue;
            };

            lookup.act_order.push(season_id.clone());
            if season.is_active == Some(true) {
                active_act_index = lookup.act_order.len().checked_sub(1);
            }
        }
    }

    if active_act_index == Some(0) {
        lookup.act_order.reverse();
    }

    lookup
}

pub async fn fetch_pregame_players(
    api: &mut RiotApiClient,
    match_id: &str,
    _config: &Config,
) -> Result<Vec<PlayerDisplayData>> {
    tracing::debug!("Fetching pregame players for match_id={}", match_id);
    let pregame = api.get_pregame_match(match_id).await?;
    api.fetch_agents().await.ok();
    api.fetch_skin_levels().await.ok();

    let mut puuids: Vec<String> = Vec::new();
    if let Some(team) = &pregame.ally_team {
        for p in &team.players {
            puuids.push(p.subject.clone());
        }
    }

    tracing::debug!("Pregame ally_team puuids: {}", puuids.len());
    let names = api.get_names(&puuids).await.unwrap_or_default();
    tracing::debug!("Pregame name service returned {} entries", names.len());
    let name_map: HashMap<String, &NameServiceEntry> =
        names.iter().map(|n| (n.subject.clone(), n)).collect();

    let mut players = Vec::new();
    if let Some(team) = &pregame.ally_team {
        for p in &team.players {
            let mut display = build_basic_player(&p.subject, &name_map);

            if let Some(identity) = &p.player_identity {
                display.account_level = identity.account_level.unwrap_or(0);
                display.is_incognito = identity.incognito.unwrap_or(false);
            }

            if let Some(char_id) = &p.character_i_d {
                display.agent_name = api.get_agent_name(char_id);
            }

            display.team_id = team.team_i_d.clone().unwrap_or_default();
            players.push(display);
        }
    }

    tracing::debug!("Built {} pregame PlayerDisplayData entries", players.len());
    Ok(players)
}

pub async fn fetch_coregame_players(
    api: &mut RiotApiClient,
    match_id: &str,
    config: &Config,
) -> Result<Vec<PlayerDisplayData>> {
    tracing::debug!("Fetching coregame players for match_id={}", match_id);
    let coregame = api.get_coregame_match(match_id).await?;
    api.fetch_agents().await.ok();
    api.fetch_skin_levels().await.ok();

    let puuids: Vec<String> = coregame.players.iter().map(|p| p.subject.clone()).collect();
    tracing::debug!("Coregame player puuids: {}", puuids.len());
    let names = api.get_names(&puuids).await.unwrap_or_default();
    tracing::debug!("Coregame name service returned {} entries", names.len());
    let name_map: HashMap<String, &NameServiceEntry> =
        names.iter().map(|n| (n.subject.clone(), n)).collect();

    let loadouts = api.get_coregame_loadouts(match_id).await.ok();
    tracing::debug!("Coregame loadouts available: {}", loadouts.is_some());

    let loadout_map: HashMap<String, &PlayerLoadout> = loadouts
        .as_ref()
        .map(|l| {
            l.loadouts
                .iter()
                .map(|lo| (lo.subject.clone(), lo))
                .collect()
        })
        .unwrap_or_default();

    let mut players = Vec::new();
    for p in &coregame.players {
        let mut display = build_basic_player(&p.subject, &name_map);

        if let Some(identity) = &p.player_identity {
            display.account_level = identity.account_level.unwrap_or(0);
            display.is_incognito = identity.incognito.unwrap_or(false);
        }

        if let Some(char_id) = &p.character_i_d {
            display.agent_name = api.get_agent_name(char_id);
        }

        display.team_id = p.team_i_d.clone().unwrap_or_default();

        if let Some(loadout) = loadout_map.get(&p.subject) {
            let skin = extract_weapon_skin(api, loadout, &config.overlay.weapon);
            display.skin_name = skin.name;
            display.skin_level = skin.level;
            display.skin_level_total = skin.total_levels;
            display.skin_color = skin.color;
        }

        players.push(display);
    }

    tracing::debug!("Built {} coregame PlayerDisplayData entries", players.len());
    Ok(players)
}

/// Marks players that share the local player's party using chat presences.
/// This ensures incognito party members have their `party_id` set so the
/// overlay can reveal their names (matching in-game behaviour).
pub async fn mark_party_from_presences(
    api: &RiotApiClient,
    players: &mut [PlayerDisplayData],
) {
    let presences = match api.get_valorant_presences().await {
        Ok(p) => p,
        Err(_) => return,
    };

    let local_puuid = api.puuid();
    let Some((_, local_presence)) = presences.iter().find(|(puuid, _)| puuid == local_puuid)
    else {
        return;
    };

    if local_presence.party_id.is_empty() || local_presence.party_size <= 1 {
        return;
    }

    let party_puuids: std::collections::HashSet<&str> = presences
        .iter()
        .filter(|(_, presence)| presence.party_id == local_presence.party_id)
        .map(|(puuid, _)| puuid.as_str())
        .collect();

    for player in players.iter_mut() {
        if party_puuids.contains(player.puuid.as_str()) {
            player.party_id = local_presence.party_id.clone();
        }
    }
}

pub async fn fetch_menu_party_players(api: &RiotApiClient) -> Result<Vec<PlayerDisplayData>> {
    let presences = api.get_valorant_presences().await?;
    let local_puuid = api.puuid();
    let Some((_, local_presence)) = presences.iter().find(|(puuid, _)| puuid == local_puuid) else {
        return Ok(Vec::new());
    };

    if local_presence.party_id.is_empty() || local_presence.party_size <= 1 {
        return Ok(Vec::new());
    }

    let mut party_puuids = Vec::new();
    party_puuids.push(local_puuid.to_string());
    party_puuids.extend(
        presences
            .iter()
            .filter(|(puuid, presence)| {
                puuid.as_str() != local_puuid && presence.party_id == local_presence.party_id
            })
            .map(|(puuid, _)| puuid.clone()),
    );

    let names = api.get_names(&party_puuids).await.unwrap_or_default();
    let name_map: HashMap<String, &NameServiceEntry> = names
        .iter()
        .map(|name| (name.subject.clone(), name))
        .collect();

    let mut players = Vec::with_capacity(party_puuids.len());
    for puuid in party_puuids {
        let mut display = build_basic_player(&puuid, &name_map);
        display.team_id = "party".to_string();
        display.party_id = local_presence.party_id.clone();
        display.party_number = 1;
        players.push(display);
    }

    Ok(players)
}

fn build_basic_player(
    puuid: &str,
    name_map: &HashMap<String, &NameServiceEntry>,
) -> PlayerDisplayData {
    let mut display = PlayerDisplayData {
        puuid: puuid.to_string(),
        ..Default::default()
    };

    if let Some(name_entry) = name_map.get(puuid) {
        display.game_name = name_entry
            .game_name
            .clone()
            .or_else(|| name_entry.display_name.clone())
            .unwrap_or_default();
        display.tag_line = name_entry.tag_line.clone().unwrap_or_default();
    }

    display
}

pub async fn enrich_player(
    api: &RiotApiClient,
    player: &mut PlayerDisplayData,
    current_season: &Option<String>,
    season_lookup: &SeasonLookup,
) {
    let puuid = player.puuid.clone();
    let short_id = &puuid[..8.min(puuid.len())];
    tracing::debug!(
        "Enriching player {} name='{}' tag='{}' season={}",
        short_id,
        player.game_name,
        player.tag_line,
        current_season.as_deref().unwrap_or("none")
    );

    let (mmr_ok, latest_comp_match_id) = match api.get_mmr(&puuid).await {
        Ok(mmr) => {
            let has_data = mmr.subject.is_some();
            if mmr.queue_skills.is_none() && has_data {
                tracing::debug!("MMR response has no queue_skills for {}", short_id);
            }
            extract_rank_data(player, &mmr, current_season, season_lookup);
            extract_latest_comp_update(player, mmr.latest_competitive_update.as_ref());
            (
                has_data,
                mmr.latest_competitive_update
                    .as_ref()
                    .and_then(|update| update.match_i_d.clone()),
            )
        }
        Err(e) => {
            tracing::warn!("Failed to fetch MMR for {}: {}", short_id, e);
            (false, None)
        }
    };

    let comp_ok = match api.get_competitive_updates(&puuid).await {
        Ok(updates) => {
            if !player.has_comp_update {
                extract_earned_rr(player, &updates);
            }
            extract_performance_from_matches(
                player,
                api,
                &updates,
                latest_comp_match_id.as_deref(),
            )
            .await;
            true
        }
        Err(e) => {
            tracing::warn!(
                "Failed to fetch competitive updates for {}: {}",
                short_id,
                e
            );
            false
        }
    };

    player.enriched = mmr_ok || comp_ok;
}

fn extract_rank_data(
    display: &mut PlayerDisplayData,
    mmr: &MmrResponse,
    current_season: &Option<String>,
    season_lookup: &SeasonLookup,
) {
    if let Some(skills) = &mmr.queue_skills {
        if let Some(comp) = skills.get("competitive") {
            if let Some(seasonal) = &comp.seasonal_info_by_season_i_d {
                let season_ids = ordered_season_ids(seasonal, season_lookup);
                let total_wins: i32 = seasonal
                    .values()
                    .map(|info| info.number_of_wins.unwrap_or(0))
                    .sum();
                let total_games: i32 = seasonal
                    .values()
                    .map(|info| info.number_of_games.unwrap_or(0))
                    .sum();

                // Current season rank
                if let Some(season_id) = current_season {
                    if let Some(info) = seasonal.get(season_id) {
                        display.current_rank = info.competitive_tier.unwrap_or(0);
                        display.rank_name = rank_name(display.current_rank).to_string();
                        display.rr = info.ranking_in_tier.unwrap_or(0);
                        display.leaderboard_position = info.leaderboard_rank.unwrap_or(0);
                        display.current_act_games = info.number_of_games.unwrap_or(0);
                    }
                }

                display.wins = total_wins;
                display.games = total_games;
                if display.games > 0 {
                    display.winrate = (display.wins as f64 / display.games as f64) * 100.0;
                }

                // Peak rank across all seasons
                let mut peak_tier = 0i32;
                let mut prev_tier = 0i32;

                for season_id in &season_ids {
                    let Some(info) = seasonal.get(season_id) else {
                        continue;
                    };

                    let mut season_peak = info.competitive_tier.unwrap_or(0);
                    if let Some(wins_by_tier) = &info.wins_by_tier {
                        for (tier_str, _) in wins_by_tier {
                            if let Ok(tier) = tier_str.parse::<i32>() {
                                season_peak = season_peak.max(tier);
                            }
                        }
                    }

                    if season_peak >= peak_tier {
                        peak_tier = season_peak;
                    }
                }

                // Previous season: the closest completed act next to the current act.
                if let Some(current_sid) = current_season.as_deref() {
                    prev_tier = previous_rank_tier(seasonal, &season_ids, current_sid);
                }

                display.peak_rank = peak_tier;
                display.peak_rank_name = rank_name(peak_tier).to_string();
                display.previous_rank = prev_tier;
                display.previous_rank_name = rank_name(prev_tier).to_string();
            }
        }
    }

    if display.rank_name.is_empty() {
        display.rank_name = rank_name(display.current_rank).to_string();
    }
}

fn extract_latest_comp_update(
    player: &mut PlayerDisplayData,
    latest_update: Option<&CompetitiveUpdate>,
) {
    if let Some(update) = latest_update {
        apply_competitive_update(player, update);
    }
}

fn ordered_season_ids(
    seasonal: &HashMap<String, SeasonalInfo>,
    season_lookup: &SeasonLookup,
) -> Vec<String> {
    let mut season_ids = Vec::new();

    for season_id in &season_lookup.act_order {
        if seasonal.contains_key(season_id) {
            season_ids.push(season_id.clone());
        }
    }

    let mut remainder: Vec<String> = seasonal
        .keys()
        .filter(|season_id| !season_lookup.act_order.contains(*season_id))
        .cloned()
        .collect();
    remainder.sort();
    season_ids.extend(remainder);
    season_ids
}

fn previous_rank_tier(
    seasonal: &HashMap<String, SeasonalInfo>,
    season_ids: &[String],
    current_season: &str,
) -> i32 {
    let Some(current_index) = season_ids
        .iter()
        .position(|season_id| season_id == current_season)
    else {
        return 0;
    };

    let search: Box<dyn Iterator<Item = &String>> = if current_index == 0 {
        Box::new(season_ids.iter().skip(1))
    } else {
        Box::new(season_ids[..current_index].iter().rev())
    };

    for season_id in search {
        let Some(info) = seasonal.get(season_id) else {
            continue;
        };

        let tier = info.competitive_tier.unwrap_or(0);
        if tier > 0 {
            return tier;
        }
    }

    0
}

fn extract_earned_rr(player: &mut PlayerDisplayData, updates: &CompetitiveUpdatesResponse) {
    let short_id = player.puuid[..8.min(player.puuid.len())].to_string();
    tracing::debug!(
        "Competitive updates for {}: {} matches",
        short_id,
        updates.matches.len()
    );

    if let Some(first) = updates.matches.first() {
        apply_competitive_update(player, first);

        tracing::debug!(
            "Comp update for {}: earned_rr={:?} resolved_earned_rr={} afk_penalty={} tier_after={:?} tier_before={:?} rr_after={:?} rr_before={:?}",
            short_id,
            first.ranked_rating_earned,
            player.earned_rr,
            player.afk_penalty,
            first.tier_after_update,
            first.tier_before_update,
            first.ranked_rating_after_update,
            first.ranked_rating_before_update
        );
    }
}

fn apply_competitive_update(player: &mut PlayerDisplayData, update: &CompetitiveUpdate) {
    player.earned_rr = earned_rr_from_update(update).unwrap_or(0);
    player.afk_penalty = update.afk_penalty.unwrap_or(0);
    player.has_comp_update = true;
}

fn earned_rr_from_update(update: &CompetitiveUpdate) -> Option<i32> {
    let afk_penalty = update.afk_penalty.unwrap_or(0);
    let derived_delta = match (
        update.ranked_rating_after_update,
        update.ranked_rating_before_update,
    ) {
        (Some(after), Some(before)) => Some((after - before) + afk_penalty),
        _ => None,
    };

    match (update.ranked_rating_earned, derived_delta) {
        (Some(earned), Some(derived)) if earned == 0 && derived != 0 => Some(derived),
        (Some(earned), _) => Some(earned),
        (None, Some(derived)) => Some(derived),
        (None, None) => None,
    }
}

async fn extract_performance_from_matches(
    display: &mut PlayerDisplayData,
    api: &RiotApiClient,
    updates: &CompetitiveUpdatesResponse,
    latest_comp_match_id: Option<&str>,
) {
    let mut match_id = preferred_recent_match_id(latest_comp_match_id, updates, None);
    if match_id.is_none() {
        let history = api.get_match_history(&display.puuid).await.ok();
        match_id = preferred_recent_match_id(latest_comp_match_id, updates, history.as_ref());
    }

    let Some(match_id) = match_id else {
        return;
    };

    if let Ok(details) = api.get_match_details(&match_id).await {
        apply_match_performance(display, &details);
    }
}

fn preferred_recent_match_id(
    latest_comp_match_id: Option<&str>,
    updates: &CompetitiveUpdatesResponse,
    fallback_history: Option<&serde_json::Value>,
) -> Option<String> {
    latest_comp_match_id
        .filter(|match_id| !match_id.is_empty())
        .map(str::to_owned)
        .or_else(|| {
            updates
                .matches
                .iter()
                .find_map(|update| update.match_i_d.as_deref())
                .filter(|match_id| !match_id.is_empty())
                .map(str::to_owned)
        })
        .or_else(|| fallback_history.and_then(latest_match_id_from_history))
}

fn latest_match_id_from_history(history: &serde_json::Value) -> Option<String> {
    history
        .get("History")?
        .as_array()?
        .iter()
        .find_map(|entry| entry.get("MatchID")?.as_str())
        .filter(|match_id| !match_id.is_empty())
        .map(str::to_owned)
}

fn apply_match_performance(display: &mut PlayerDisplayData, details: &MatchDetailsResponse) {
    if let Some(performance) = extract_player_performance(details, &display.puuid) {
        display.kd = performance.kd;
    }

    let (hs, bs, ls) = aggregate_damage(details, &display.puuid);
    let total = hs + bs + ls;
    if total > 0 {
        display.headshot_percent = (hs as f64 / total as f64 * 100.0 * 10.0).round() / 10.0;
    }
}

fn aggregate_damage(details: &MatchDetailsResponse, puuid: &str) -> (i32, i32, i32) {
    let mut hs = 0i32;
    let mut bs = 0i32;
    let mut ls = 0i32;

    if let Some(rounds) = &details.round_results {
        for round in rounds {
            if let Some(stats) = &round.player_stats {
                for ps in stats {
                    if ps.subject == puuid {
                        if let Some(damages) = &ps.damage {
                            for d in damages {
                                hs += d.headshots.unwrap_or(0);
                                bs += d.bodyshots.unwrap_or(0);
                                ls += d.legshots.unwrap_or(0);
                            }
                        }
                    }
                }
            }
        }
    }

    (hs, bs, ls)
}

fn extract_weapon_skin(
    api: &RiotApiClient,
    loadout: &PlayerLoadout,
    weapon_name: &str,
) -> EquippedSkin {
    let weapon_name = normalize_overlay_weapon(weapon_name);
    let weapon_uuid = overlay_weapon_uuid(weapon_name);

    if let Some(items) = loadout.items() {
        if let Some(item) = items.get(weapon_uuid) {
            if let Some(sockets) = &item.sockets {
                for socket in sockets.values() {
                    if let Some(socket_item) = &socket.item {
                        if let Some(id) = &socket_item.i_d {
                            if let Some(skin) = api.get_skin_level_info(id) {
                                return EquippedSkin {
                                    name: skin.skin_name.clone(),
                                    level: skin.level,
                                    total_levels: skin.total_levels,
                                    color: skin.color,
                                };
                            }

                            let name = api.get_skin_name(id);
                            if name != "Unknown" {
                                return EquippedSkin {
                                    name,
                                    color: skin_tier_color(None),
                                    ..Default::default()
                                };
                            }
                        }
                    }
                }
            }
        }
    }

    EquippedSkin {
        name: standard_skin_name(weapon_name),
        color: skin_tier_color(None),
        ..Default::default()
    }
}

pub fn normalize_overlay_weapon(name: &str) -> &'static str {
    let trimmed = name.trim();
    OVERLAY_WEAPONS
        .iter()
        .copied()
        .find(|option| option.eq_ignore_ascii_case(trimmed))
        .unwrap_or("Vandal")
}

fn overlay_weapon_uuid(weapon_name: &str) -> &'static str {
    match normalize_overlay_weapon(weapon_name) {
        "Vandal" => "9c82e19d-4575-0200-1a81-3eacf00cf872",
        "Phantom" => "ee8e8d15-496b-07ac-f604-8f8488911e76",
        "Operator" => "a03b24d3-4319-996d-0f8c-94bbfba1dfc7",
        "Sheriff" => "e336c6b8-418d-9340-d77f-7a9e4cfe0702",
        "Spectre" => "462080d1-4035-2937-7c09-27aa2a5c27a7",
        "Classic" => "29a0cfab-485b-f5d5-779a-b59f85e204a8",
        _ => "9c82e19d-4575-0200-1a81-3eacf00cf872",
    }
}

fn standard_skin_name(weapon_name: &str) -> String {
    let weapon_name = weapon_name.trim();
    if weapon_name.is_empty() {
        "Standard".to_string()
    } else {
        format!("Standard {weapon_name}")
    }
}

#[cfg(test)]
mod tests {
    use super::{
        apply_competitive_update, build_season_lookup, earned_rr_from_update,
        extract_latest_comp_update, extract_rank_data, normalize_overlay_weapon,
        preferred_recent_match_id, SeasonLookup,
    };
    use crate::riot::types::{
        CompetitiveUpdate, CompetitiveUpdatesResponse, ContentResponse, ContentSeason, MmrResponse,
        PlayerDisplayData, QueueSkill, SeasonalInfo,
    };
    use serde_json::json;
    use std::collections::HashMap;

    #[test]
    fn finds_previous_rank_when_current_act_is_last() {
        let mut seasonal = HashMap::new();
        seasonal.insert(
            "act-1".to_string(),
            SeasonalInfo {
                competitive_tier: Some(17),
                ..Default::default()
            },
        );
        seasonal.insert(
            "act-2".to_string(),
            SeasonalInfo {
                competitive_tier: Some(20),
                ..Default::default()
            },
        );

        let mmr = MmrResponse {
            queue_skills: Some(HashMap::from([(
                "competitive".to_string(),
                QueueSkill {
                    seasonal_info_by_season_i_d: Some(seasonal),
                    ..Default::default()
                },
            )])),
            ..Default::default()
        };

        let lookup = build_season_lookup(&ContentResponse {
            seasons: Some(vec![
                ContentSeason {
                    i_d: Some("act-1".to_string()),
                    name: Some("Episode 8 Act 3".to_string()),
                    is_active: Some(false),
                    season_type: Some("act".to_string()),
                },
                ContentSeason {
                    i_d: Some("act-2".to_string()),
                    name: Some("Episode 9 Act 1".to_string()),
                    is_active: Some(true),
                    season_type: Some("act".to_string()),
                },
            ]),
        });

        let mut player = PlayerDisplayData::default();
        extract_rank_data(&mut player, &mmr, &Some("act-2".to_string()), &lookup);

        assert_eq!(player.peak_rank, 20);
        assert_eq!(player.previous_rank, 17);
    }

    #[test]
    fn winrate_uses_total_comp_history_and_tracks_current_act_games() {
        let mut seasonal = HashMap::new();
        seasonal.insert(
            "act-1".to_string(),
            SeasonalInfo {
                number_of_wins: Some(18),
                number_of_games: Some(30),
                competitive_tier: Some(17),
                ..Default::default()
            },
        );
        seasonal.insert(
            "act-2".to_string(),
            SeasonalInfo {
                number_of_wins: Some(10),
                number_of_games: Some(20),
                competitive_tier: Some(20),
                ..Default::default()
            },
        );

        let mmr = MmrResponse {
            queue_skills: Some(HashMap::from([(
                "competitive".to_string(),
                QueueSkill {
                    seasonal_info_by_season_i_d: Some(seasonal),
                    ..Default::default()
                },
            )])),
            ..Default::default()
        };

        let lookup = build_season_lookup(&ContentResponse {
            seasons: Some(vec![
                ContentSeason {
                    i_d: Some("act-1".to_string()),
                    name: Some("Episode 8 Act 3".to_string()),
                    is_active: Some(false),
                    season_type: Some("act".to_string()),
                },
                ContentSeason {
                    i_d: Some("act-2".to_string()),
                    name: Some("Episode 9 Act 1".to_string()),
                    is_active: Some(true),
                    season_type: Some("act".to_string()),
                },
            ]),
        });

        let mut player = PlayerDisplayData::default();
        extract_rank_data(&mut player, &mmr, &Some("act-2".to_string()), &lookup);

        assert_eq!(player.wins, 28);
        assert_eq!(player.games, 50);
        assert_eq!(player.current_act_games, 20);
        assert!((player.winrate - 56.0).abs() < 1e-9);
    }

    #[test]
    fn finds_previous_rank_when_current_act_is_first() {
        let mut seasonal = HashMap::new();
        seasonal.insert(
            "act-1".to_string(),
            SeasonalInfo {
                competitive_tier: Some(17),
                ..Default::default()
            },
        );
        seasonal.insert(
            "act-2".to_string(),
            SeasonalInfo {
                competitive_tier: Some(20),
                ..Default::default()
            },
        );

        let mmr = MmrResponse {
            queue_skills: Some(HashMap::from([(
                "competitive".to_string(),
                QueueSkill {
                    seasonal_info_by_season_i_d: Some(seasonal),
                    ..Default::default()
                },
            )])),
            ..Default::default()
        };

        let lookup = build_season_lookup(&ContentResponse {
            seasons: Some(vec![
                ContentSeason {
                    i_d: Some("act-2".to_string()),
                    name: Some("Episode 9 Act 1".to_string()),
                    is_active: Some(true),
                    season_type: Some("act".to_string()),
                },
                ContentSeason {
                    i_d: Some("act-1".to_string()),
                    name: Some("Episode 8 Act 3".to_string()),
                    is_active: Some(false),
                    season_type: Some("act".to_string()),
                },
            ]),
        });

        let mut player = PlayerDisplayData::default();
        extract_rank_data(&mut player, &mmr, &Some("act-2".to_string()), &lookup);

        assert_eq!(player.previous_rank, 17);
    }

    #[test]
    fn falls_back_when_no_content_lookup_exists() {
        let lookup = SeasonLookup::default();
        let mut seasonal = HashMap::new();
        seasonal.insert(
            "zzz".to_string(),
            SeasonalInfo {
                competitive_tier: Some(12),
                ..Default::default()
            },
        );

        let mmr = MmrResponse {
            queue_skills: Some(HashMap::from([(
                "competitive".to_string(),
                QueueSkill {
                    seasonal_info_by_season_i_d: Some(seasonal),
                    ..Default::default()
                },
            )])),
            ..Default::default()
        };

        let mut player = PlayerDisplayData::default();
        extract_rank_data(&mut player, &mmr, &None, &lookup);

        assert_eq!(player.peak_rank, 12);
        assert_eq!(player.previous_rank, 0);
    }

    #[test]
    fn normalizes_overlay_weapon_choices() {
        assert_eq!(normalize_overlay_weapon("phantom"), "Phantom");
        assert_eq!(normalize_overlay_weapon("unknown"), "Vandal");
    }

    #[test]
    fn keeps_direct_earned_rr_when_present() {
        let update = CompetitiveUpdate {
            ranked_rating_earned: Some(18),
            ranked_rating_after_update: Some(62),
            ranked_rating_before_update: Some(44),
            afk_penalty: Some(0),
            ..Default::default()
        };

        assert_eq!(earned_rr_from_update(&update), Some(18));
    }

    #[test]
    fn reconstructs_earned_rr_from_progress_when_direct_value_is_zero() {
        let update = CompetitiveUpdate {
            ranked_rating_earned: Some(0),
            ranked_rating_after_update: Some(71),
            ranked_rating_before_update: Some(55),
            afk_penalty: Some(0),
            ..Default::default()
        };

        assert_eq!(earned_rr_from_update(&update), Some(16));
    }

    #[test]
    fn reconstructs_pre_penalty_rr_from_progress_and_afk_penalty() {
        let update = CompetitiveUpdate {
            ranked_rating_earned: Some(0),
            ranked_rating_after_update: Some(42),
            ranked_rating_before_update: Some(50),
            afk_penalty: Some(8),
            ..Default::default()
        };

        assert_eq!(earned_rr_from_update(&update), Some(0));
    }

    #[test]
    fn extracts_delta_rr_from_latest_mmr_update() {
        let mut player = PlayerDisplayData::default();
        let update = CompetitiveUpdate {
            ranked_rating_earned: Some(-17),
            afk_penalty: Some(0),
            ..Default::default()
        };

        extract_latest_comp_update(&mut player, Some(&update));

        assert!(player.has_comp_update);
        assert_eq!(player.earned_rr, -17);
        assert_eq!(player.afk_penalty, 0);
    }

    #[test]
    fn apply_competitive_update_reconstructs_from_progress() {
        let mut player = PlayerDisplayData::default();
        let update = CompetitiveUpdate {
            ranked_rating_earned: Some(0),
            ranked_rating_after_update: Some(63),
            ranked_rating_before_update: Some(44),
            afk_penalty: Some(0),
            ..Default::default()
        };

        apply_competitive_update(&mut player, &update);

        assert!(player.has_comp_update);
        assert_eq!(player.earned_rr, 19);
    }

    #[test]
    fn preferred_recent_match_id_prefers_latest_comp_match() {
        let updates = CompetitiveUpdatesResponse {
            matches: vec![CompetitiveUpdate {
                match_i_d: Some("comp-from-updates".into()),
                ..Default::default()
            }],
            subject: None,
        };
        let history = json!({
            "History": [
                { "MatchID": "latest-overall" }
            ]
        });

        let match_id =
            preferred_recent_match_id(Some("latest-comp"), &updates, Some(&history)).unwrap();

        assert_eq!(match_id, "latest-comp");
    }

    #[test]
    fn preferred_recent_match_id_falls_back_to_latest_match_history() {
        let updates = CompetitiveUpdatesResponse {
            matches: Vec::new(),
            subject: None,
        };
        let history = json!({
            "History": [
                { "MatchID": "latest-overall" }
            ]
        });

        let match_id = preferred_recent_match_id(None, &updates, Some(&history)).unwrap();

        assert_eq!(match_id, "latest-overall");
    }
}
