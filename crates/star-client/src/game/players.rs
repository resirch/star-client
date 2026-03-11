use crate::config::Config;
use crate::riot::api::RiotApiClient;
use crate::riot::types::*;
use anyhow::Result;
use std::collections::HashMap;

#[derive(Debug, Default)]
struct EquippedSkin {
    name: String,
    level: usize,
    total_levels: usize,
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
            extract_rank_data(player, &mmr, current_season);
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
) {
    if let Some(skills) = &mmr.queue_skills {
        if let Some(comp) = skills.get("competitive") {
            if let Some(seasonal) = &comp.seasonal_info_by_season_i_d {
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
                let season_ids: Vec<&String> = seasonal.keys().collect();

                for (_sid, info) in seasonal.iter() {
                    if let Some(wins_by_tier) = &info.wins_by_tier {
                        for (tier_str, _) in wins_by_tier {
                            if let Ok(tier) = tier_str.parse::<i32>() {
                                if tier > peak_tier {
                                    peak_tier = tier;
                                }
                            }
                        }
                    }
                    let tier = info.competitive_tier.unwrap_or(0);
                    if tier > peak_tier {
                        peak_tier = tier;
                    }
                }

                // Previous season: find the most recent non-current season
                if let Some(current_sid) = current_season {
                    for sid in season_ids.iter().rev() {
                        if *sid != current_sid {
                            if let Some(info) = seasonal.get(*sid) {
                                prev_tier = info.competitive_tier.unwrap_or(0);
                                if prev_tier > 0 {
                                    break;
                                }
                            }
                        }
                    }
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

fn extract_earned_rr(display: &mut PlayerDisplayData, updates: &CompetitiveUpdatesResponse) {
    let short_id = &display.puuid[..8.min(display.puuid.len())];
    tracing::debug!(
        "Competitive updates for {}: {} matches",
        short_id,
        updates.matches.len()
    );
    if let Some(first) = updates.matches.first() {
        display.earned_rr = first.ranked_rating_earned.unwrap_or(0);
        display.afk_penalty = first.afk_penalty.unwrap_or(0);
        display.has_comp_update = true;
        tracing::debug!(
            "Comp update for {}: earned_rr={:?} tier_after={:?} tier_before={:?} rr_after={:?} rr_before={:?}",
            short_id,
            first.ranked_rating_earned,
            first.tier_after_update,
            first.tier_before_update,
            first.ranked_rating_after_update,
            first.ranked_rating_before_update
        );
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
    let weapon_uuid = match weapon_name.to_lowercase().as_str() {
        "vandal" => "9c82e19d-4575-0200-1a81-3eacf00cf872",
        "phantom" => "ee8e8d15-496b-07ac-f604-8f8488911e76",
        "operator" => "a03b24d3-4319-996d-0f8c-94bbfba1dfc7",
        "sheriff" => "e336c6b8-418d-9340-d77f-7a9e4cfe0702",
        "spectre" => "462080d1-4035-2937-7c09-27aa2a5c27a7",
        "classic" => "29a0cfab-485b-f5d5-779a-b59f85e204a8",
        _ => "9c82e19d-4575-0200-1a81-3eacf00cf872", // Default to Vandal
    };

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
                                };
                            }

                            let name = api.get_skin_name(id);
                            if name != "Unknown" {
                                return EquippedSkin {
                                    name,
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
        ..Default::default()
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
