use crate::config::Config;
use crate::riot::api::RiotApiClient;
use crate::riot::types::*;
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

    let mmr_ok = match api.get_mmr(&puuid).await {
        Ok(mmr) => {
            let has_data = mmr.subject.is_some();
            if mmr.queue_skills.is_none() && has_data {
                tracing::debug!("MMR response has no queue_skills for {}", short_id);
            }
            extract_rank_data(player, &mmr, current_season, season_lookup);
            has_data
        }
        Err(e) => {
            tracing::warn!("Failed to fetch MMR for {}: {}", short_id, e);
            false
        }
    };

    let comp_ok = match api.get_competitive_updates(&puuid).await {
        Ok(updates) => {
            extract_earned_rr(player, &updates);
            extract_performance_from_updates(player, api, &updates).await;
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

                // Current season rank
                if let Some(season_id) = current_season {
                    if let Some(info) = seasonal.get(season_id) {
                        display.current_rank = info.competitive_tier.unwrap_or(0);
                        display.rank_name = rank_name(display.current_rank).to_string();
                        display.rr = info.ranking_in_tier.unwrap_or(0);
                        display.leaderboard_position = info.leaderboard_rank.unwrap_or(0);
                        display.wins = info.number_of_wins.unwrap_or(0);
                        display.games = info.number_of_games.unwrap_or(0);
                        if display.games > 0 {
                            display.winrate = (display.wins as f64 / display.games as f64) * 100.0;
                        }
                    }
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
    let short_id = &player.puuid[..8.min(player.puuid.len())];
    tracing::debug!(
        "Competitive updates for {}: {} matches",
        short_id,
        updates.matches.len()
    );

    if let Some(first) = updates.matches.first() {
        player.earned_rr = earned_rr_from_update(first).unwrap_or(0);
        player.afk_penalty = first.afk_penalty.unwrap_or(0);
        player.has_comp_update = true;

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

async fn extract_performance_from_updates(
    display: &mut PlayerDisplayData,
    api: &RiotApiClient,
    updates: &CompetitiveUpdatesResponse,
) {
    if let Some(first) = updates.matches.first() {
        if let Some(match_id) = &first.match_i_d {
            if let Ok(details) = api.get_match_details(match_id).await {
                for player in &details.players {
                    if player.subject == display.puuid {
                        if let Some(stats) = &player.stats {
                            let kills = stats.kills.unwrap_or(0) as f64;
                            let deaths = stats.deaths.unwrap_or(1).max(1) as f64;
                            display.kd = (kills / deaths * 100.0).round() / 100.0;
                        }
                    }
                }

                let (hs, bs, ls) = aggregate_damage(&details, &display.puuid);
                let total = hs + bs + ls;
                if total > 0 {
                    display.headshot_percent =
                        (hs as f64 / total as f64 * 100.0 * 10.0).round() / 10.0;
                }
            }
        }
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
        build_season_lookup, earned_rr_from_update, extract_rank_data, normalize_overlay_weapon,
        SeasonLookup,
    };
    use crate::riot::types::{
        CompetitiveUpdate, ContentResponse, ContentSeason, MmrResponse, PlayerDisplayData,
        QueueSkill, SeasonalInfo,
    };
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
}
