//! Party detection for players the match payload does not group.
//!
//! Riot's live pregame/core-game payloads only sometimes expose party IDs, and
//! presence only covers the local player's own party. For everyone else we
//! look at their recent completed matches:
//!
//! 1. Matches two same-team players both played are *candidates*. Being in the
//!    same lobby alone proves nothing: short queues (Swiftplay, Spike Rush,
//!    Deathmatch), high ranks and low-population servers keep putting the same
//!    solo players together.
//! 2. Completed match details carry each player's real `partyId`, so for the
//!    newest shared matches we check whether the pair was actually partied.
//! 3. A pair is only linked when they were partied in their most recent shared
//!    match, recently, and neither has played many team games without the
//!    other since. Solo Deathmatch warm-ups don't count against them.
//! 4. Links are clustered under queue rules: at most five per party and never
//!    a four-stack in Competitive.

use crate::riot::api::RiotApiClient;
use crate::riot::types::PlayerDisplayData;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Recent matches pulled per player when looking for shared games.
const HISTORY_DEPTH: usize = 10;
/// Newest shared matches per pair whose party IDs get checked.
const SHARED_MATCHES_PER_PAIR: usize = 2;
/// Upper bound on match-details requests per lookup.
const MAX_DETAIL_FETCHES: usize = 10;
/// Largest party any matchmade queue allows.
const MAX_PARTY_SIZE: usize = 5;
/// Histories are re-used across the pregame/in-game refreshes of one match.
const HISTORY_CACHE_TTL: Duration = Duration::from_secs(120);
/// Completed match details never change, so their party maps are cached.
const MATCH_PARTY_CACHE_LIMIT: usize = 256;
const HOUR_MS: i64 = 60 * 60 * 1000;

/// Legacy lobby-overlap heuristic, used only for pairs whose shared matches
/// could not be fetched (rate limits, timeouts).
const FALLBACK_WINDOW: usize = 5;
const FALLBACK_SHARED: usize = 4;

/// Assigns overlay party numbers from match-provided party IDs.
/// Solo unique IDs are cleared so history detection can still fill them in.
pub fn apply_match_party_ids(players: &mut [PlayerDisplayData]) {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for player in players.iter() {
        if !player.party_id.is_empty() {
            *counts.entry(player.party_id.clone()).or_insert(0) += 1;
        }
    }

    let mut stacked: Vec<String> = counts
        .into_iter()
        .filter(|(_, count)| *count >= 2)
        .map(|(id, _)| id)
        .collect();
    stacked.sort();

    let mut party_num = next_party_number(players);
    for party_id in stacked {
        for player in players.iter_mut() {
            if player.party_id == party_id && player.party_number <= 0 {
                player.party_number = party_num;
            }
        }
        party_num += 1;
    }

    for player in players.iter_mut() {
        if player.party_number <= 0 {
            player.party_id.clear();
        }
    }
}

/// Detects non-local parties from the party IDs of recently shared matches.
/// Players who already have a party assignment are left untouched, and the
/// local player is skipped because presence exposes their real party.
pub async fn detect_parties(api: &RiotApiClient, players: &mut [PlayerDisplayData]) {
    let local_puuid = api.puuid().to_string();
    let presence = api
        .get_self_presence()
        .await
        .ok()
        .flatten()
        .unwrap_or_default();
    let rules = queue_rules(&presence.queue_id, &presence.provisioning_flow);
    if rules.skip {
        tracing::debug!(
            "Party finder skipped for queue '{}' ({})",
            presence.queue_id,
            presence.provisioning_flow
        );
        return;
    }
    let free_for_all = rules.free_for_all || is_free_for_all_roster(players);

    let targets: Vec<&PlayerDisplayData> = players
        .iter()
        .filter(|player| player.puuid != local_puuid && !player_already_in_party(player))
        .collect();
    if targets.len() < 2 {
        return;
    }

    let puuids: Vec<String> = targets.iter().map(|player| player.puuid.clone()).collect();
    let histories = fetch_histories(api, &puuids).await;
    let pairs = candidate_pairs(&targets, &histories, free_for_all);
    if pairs.is_empty() {
        return;
    }

    let wanted = matches_to_resolve(&pairs, MAX_DETAIL_FETCHES);
    let resolved = resolve_match_parties(api, &wanted).await;
    let now_ms = now_ms();

    let edges: Vec<Edge> = pairs
        .iter()
        .filter_map(|pair| {
            let strength = link_strength(
                pair,
                histories.get(&pair.a)?,
                histories.get(&pair.b)?,
                &resolved,
                now_ms,
            )?;
            Some(Edge {
                a: pair.a.clone(),
                b: pair.b.clone(),
                strength,
            })
        })
        .collect();

    let roster_order: HashMap<String, usize> = players
        .iter()
        .enumerate()
        .map(|(index, player)| (player.puuid.clone(), index))
        .collect();
    let groups = cluster(edges, rules, &roster_order);
    tracing::debug!(
        "Party finder: {} candidate pairs, {}/{} shared matches resolved, {} groups",
        pairs.len(),
        resolved.len(),
        wanted.len(),
        groups.len()
    );

    assign_party_numbers(players, &groups, &local_puuid);
}

// --- Queue rules ---

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct QueueRules {
    /// No matchmade parties to infer (custom lobbies, Premier rosters).
    skip: bool,
    /// Everyone is on their own team, so teammates can't narrow candidates.
    free_for_all: bool,
    /// Competitive allows parties of 1, 2, 3 or 5, never 4.
    no_four_stacks: bool,
}

fn queue_rules(queue_id: &str, provisioning_flow: &str) -> QueueRules {
    let queue = queue_id.to_ascii_lowercase();
    let flow = provisioning_flow.to_ascii_lowercase();
    QueueRules {
        skip: flow.contains("custom") || queue == "premier",
        free_for_all: is_warmup_queue(&queue),
        no_four_stacks: queue == "competitive",
    }
}

/// Queues whose match party IDs say nothing about who queued together:
/// a custom lobby shares one party, and Premier teams are fixed rosters.
fn is_party_evidence_queue(queue_id: &str) -> bool {
    !queue_id.is_empty() && queue_id != "premier" && !queue_id.contains("custom")
}

/// Solo free-for-all queues that players often run between party games.
fn is_warmup_queue(queue_id: &str) -> bool {
    queue_id == "deathmatch"
}

fn is_free_for_all_roster(players: &[PlayerDisplayData]) -> bool {
    let team_ids: Vec<&str> = players
        .iter()
        .map(|player| player.team_id.as_str())
        .filter(|id| !id.is_empty())
        .collect();
    team_ids.len() > 2 && team_ids.iter().collect::<HashSet<_>>().len() == team_ids.len()
}

// --- Match history ---

#[derive(Debug, Clone, PartialEq, Eq)]
struct HistoryEntry {
    match_id: String,
    queue_id: String,
    start_ms: i64,
}

type History = Arc<Vec<HistoryEntry>>;

fn parse_history(value: &serde_json::Value) -> Vec<HistoryEntry> {
    value["History"]
        .as_array()
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| {
                    let match_id = entry["MatchID"].as_str().filter(|id| !id.is_empty())?;
                    Some(HistoryEntry {
                        match_id: match_id.to_string(),
                        queue_id: entry["QueueID"]
                            .as_str()
                            .unwrap_or_default()
                            .to_ascii_lowercase(),
                        start_ms: entry["GameStartTime"].as_i64().unwrap_or(0),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn history_cache() -> &'static Mutex<HashMap<String, (Instant, History)>> {
    static CACHE: OnceLock<Mutex<HashMap<String, (Instant, History)>>> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}

async fn fetch_histories(api: &RiotApiClient, puuids: &[String]) -> HashMap<String, History> {
    let mut histories = HashMap::new();
    let mut missing = Vec::new();
    {
        let cache = history_cache().lock().unwrap_or_else(|e| e.into_inner());
        for puuid in puuids {
            match cache.get(puuid) {
                Some((fetched_at, history)) if fetched_at.elapsed() < HISTORY_CACHE_TTL => {
                    histories.insert(puuid.clone(), history.clone());
                }
                _ => missing.push(puuid.clone()),
            }
        }
    }

    let results = futures_util::future::join_all(
        missing
            .iter()
            .map(|puuid| api.get_match_history_depth(puuid, HISTORY_DEPTH)),
    )
    .await;

    let mut cache = history_cache().lock().unwrap_or_else(|e| e.into_inner());
    cache.retain(|_, (fetched_at, _)| fetched_at.elapsed() < HISTORY_CACHE_TTL);
    for (puuid, result) in missing.into_iter().zip(results) {
        match result {
            Ok(value) => {
                let history: History = Arc::new(parse_history(&value));
                cache.insert(puuid.clone(), (Instant::now(), history.clone()));
                histories.insert(puuid, history);
            }
            Err(error) => tracing::debug!("Match history lookup failed for {puuid}: {error}"),
        }
    }
    histories
}

/// Team-mode matches a player finished after `match_id` (history is newest
/// first). Solo warm-ups don't count as playing without their party.
fn matches_played_since(history: &[HistoryEntry], match_id: &str) -> usize {
    history
        .iter()
        .take_while(|entry| entry.match_id != match_id)
        .filter(|entry| !is_warmup_queue(&entry.queue_id))
        .count()
}

// --- Candidates and evidence ---

#[derive(Debug, Clone, PartialEq, Eq)]
struct CandidatePair {
    a: String,
    b: String,
    /// Shared matchmade matches, newest first.
    shared: Vec<String>,
}

fn could_share_party(a: &PlayerDisplayData, b: &PlayerDisplayData, free_for_all: bool) -> bool {
    free_for_all || a.team_id.is_empty() || b.team_id.is_empty() || a.team_id == b.team_id
}

fn candidate_pairs(
    targets: &[&PlayerDisplayData],
    histories: &HashMap<String, History>,
    free_for_all: bool,
) -> Vec<CandidatePair> {
    let mut pairs = Vec::new();
    for (index, a) in targets.iter().enumerate() {
        let Some(history_a) = histories.get(&a.puuid) else {
            continue;
        };
        for b in &targets[index + 1..] {
            if !could_share_party(a, b, free_for_all) {
                continue;
            }
            let Some(history_b) = histories.get(&b.puuid) else {
                continue;
            };
            let in_b: HashSet<&str> = history_b
                .iter()
                .map(|entry| entry.match_id.as_str())
                .collect();
            let shared: Vec<String> = history_a
                .iter()
                .filter(|entry| is_party_evidence_queue(&entry.queue_id))
                .filter(|entry| in_b.contains(entry.match_id.as_str()))
                .map(|entry| entry.match_id.clone())
                .collect();
            if !shared.is_empty() {
                pairs.push(CandidatePair {
                    a: a.puuid.clone(),
                    b: b.puuid.clone(),
                    shared,
                });
            }
        }
    }
    pairs
}

/// Picks which shared matches to look up: every pair's newest shared match
/// first, then matches that answer the most pairs at once.
fn matches_to_resolve(pairs: &[CandidatePair], limit: usize) -> Vec<String> {
    let mut coverage: HashMap<&str, (usize, usize)> = HashMap::new();
    for pair in pairs {
        for (rank, match_id) in pair.shared.iter().take(SHARED_MATCHES_PER_PAIR).enumerate() {
            let entry = coverage.entry(match_id.as_str()).or_insert((rank, 0));
            entry.0 = entry.0.min(rank);
            entry.1 += 1;
        }
    }
    let mut ranked: Vec<(&str, (usize, usize))> = coverage.into_iter().collect();
    ranked.sort_by(|(id_x, (rank_x, pairs_x)), (id_y, (rank_y, pairs_y))| {
        rank_x
            .cmp(rank_y)
            .then(pairs_y.cmp(pairs_x))
            .then(id_x.cmp(id_y))
    });
    ranked
        .into_iter()
        .take(limit)
        .map(|(match_id, _)| match_id.to_string())
        .collect()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MatchPartyRoster {
    #[serde(default)]
    players: Vec<MatchPartyPlayer>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MatchPartyPlayer {
    subject: String,
    #[serde(default)]
    party_id: Option<String>,
}

/// PUUID -> party ID for one completed match.
type MatchParties = Arc<HashMap<String, String>>;

fn match_party_cache() -> &'static Mutex<HashMap<String, MatchParties>> {
    static CACHE: OnceLock<Mutex<HashMap<String, MatchParties>>> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}

async fn resolve_match_parties(
    api: &RiotApiClient,
    match_ids: &[String],
) -> HashMap<String, MatchParties> {
    let mut resolved = HashMap::new();
    let mut missing = Vec::new();
    {
        let cache = match_party_cache().lock().unwrap_or_else(|e| e.into_inner());
        for match_id in match_ids {
            match cache.get(match_id) {
                Some(parties) => {
                    resolved.insert(match_id.clone(), parties.clone());
                }
                None => missing.push(match_id.clone()),
            }
        }
    }

    let results = futures_util::future::join_all(
        missing
            .iter()
            .map(|match_id| api.get_match_details_as::<MatchPartyRoster>(match_id)),
    )
    .await;

    let mut cache = match_party_cache().lock().unwrap_or_else(|e| e.into_inner());
    if cache.len() + missing.len() > MATCH_PARTY_CACHE_LIMIT {
        cache.clear();
    }
    for (match_id, result) in missing.into_iter().zip(results) {
        match result {
            Ok(roster) => {
                let parties: MatchParties = Arc::new(
                    roster
                        .players
                        .into_iter()
                        .filter_map(|player| {
                            let party_id = player.party_id.filter(|id| !id.is_empty())?;
                            Some((player.subject, party_id))
                        })
                        .collect(),
                );
                cache.insert(match_id.clone(), parties.clone());
                resolved.insert(match_id, parties);
            }
            Err(error) => tracing::debug!("Match party lookup failed for {match_id}: {error}"),
        }
    }
    resolved
}

/// How confident a pair link is; stronger links are clustered first.
type Strength = u8;
const STRONG: Strength = 3;
const LIKELY: Strength = 2;
const WEAK: Strength = 1;

fn link_strength(
    pair: &CandidatePair,
    history_a: &[HistoryEntry],
    history_b: &[HistoryEntry],
    resolved: &HashMap<String, MatchParties>,
    now_ms: i64,
) -> Option<Strength> {
    let mut latest: Option<(&str, bool)> = None;
    let mut times_partied = 0usize;
    for match_id in &pair.shared {
        let Some(parties) = resolved.get(match_id) else {
            continue;
        };
        let (Some(party_a), Some(party_b)) = (parties.get(&pair.a), parties.get(&pair.b)) else {
            continue;
        };
        let partied = party_a == party_b;
        latest.get_or_insert((match_id.as_str(), partied));
        if partied {
            times_partied += 1;
        }
    }

    let Some((match_id, partied)) = latest else {
        // No party IDs available for any shared match: fall back to the old
        // lobby-overlap heuristic, which is at best a weak link.
        return fallback_overlap(history_a, history_b).then_some(WEAK);
    };
    if !partied {
        // Most recently seen together but queued separately: regulars on the
        // same server or rank, not a party.
        return None;
    }

    let since = matches_played_since(history_a, match_id)
        .max(matches_played_since(history_b, match_id));
    let age_hours = history_a
        .iter()
        .find(|entry| entry.match_id == match_id)
        .map(|entry| entry.start_ms)
        .filter(|start_ms| *start_ms > 0 && now_ms > 0)
        .map(|start_ms| (now_ms - start_ms).max(0) / HOUR_MS)
        .unwrap_or(0);

    if since == 0 && age_hours <= 24 {
        Some(STRONG)
    } else if since <= 1 && age_hours <= 12 {
        Some(LIKELY)
    } else if times_partied >= 2 && since <= 3 && age_hours <= 72 {
        Some(WEAK)
    } else {
        None
    }
}

fn fallback_overlap(history_a: &[HistoryEntry], history_b: &[HistoryEntry]) -> bool {
    let recent = |history: &[HistoryEntry]| -> Vec<String> {
        history
            .iter()
            .filter(|entry| is_party_evidence_queue(&entry.queue_id))
            .filter(|entry| !is_warmup_queue(&entry.queue_id))
            .take(FALLBACK_WINDOW)
            .map(|entry| entry.match_id.clone())
            .collect()
    };
    let recent_b: HashSet<String> = recent(history_b).into_iter().collect();
    recent(history_a)
        .iter()
        .filter(|match_id| recent_b.contains(*match_id))
        .count()
        >= FALLBACK_SHARED
}

// --- Clustering ---

#[derive(Debug, Clone)]
struct Edge {
    a: String,
    b: String,
    strength: Strength,
}

/// Merges linked pairs into parties, strongest links first, refusing any merge
/// the queue could not have matchmade.
fn cluster(
    mut edges: Vec<Edge>,
    rules: QueueRules,
    roster_order: &HashMap<String, usize>,
) -> Vec<HashSet<String>> {
    edges.sort_by(|x, y| {
        y.strength
            .cmp(&x.strength)
            .then_with(|| x.a.cmp(&y.a))
            .then_with(|| x.b.cmp(&y.b))
    });

    let mut group_of: HashMap<String, usize> = HashMap::new();
    let mut groups: Vec<HashSet<String>> = Vec::new();
    // A link that would make a Competitive four-stack is deferred rather than
    // dropped: once its far side becomes a duo, it can complete a five-stack.
    let mut pending = edges;
    loop {
        let mut deferred = Vec::new();
        let mut merged_any = false;
        for edge in pending {
            let group_a = group_of.get(&edge.a).copied();
            let group_b = group_of.get(&edge.b).copied();
            if group_a.is_some() && group_a == group_b {
                continue;
            }
            let merged_size =
                group_a.map_or(1, |g| groups[g].len()) + group_b.map_or(1, |g| groups[g].len());
            if merged_size > MAX_PARTY_SIZE {
                continue;
            }
            if rules.no_four_stacks && merged_size == 4 {
                deferred.push(edge);
                continue;
            }

            let target = match (group_a, group_b) {
                (Some(group), None) | (None, Some(group)) => group,
                (Some(group), Some(other)) => {
                    let moved = std::mem::take(&mut groups[other]);
                    for puuid in &moved {
                        group_of.insert(puuid.clone(), group);
                    }
                    groups[group].extend(moved);
                    group
                }
                (None, None) => {
                    groups.push(HashSet::new());
                    groups.len() - 1
                }
            };
            for puuid in [edge.a, edge.b] {
                group_of.insert(puuid.clone(), target);
                groups[target].insert(puuid);
            }
            merged_any = true;
        }
        if !merged_any || deferred.is_empty() {
            break;
        }
        pending = deferred;
    }

    let first_seat = |group: &HashSet<String>| {
        group
            .iter()
            .filter_map(|puuid| roster_order.get(puuid).copied())
            .min()
            .unwrap_or(usize::MAX)
    };
    let mut groups: Vec<HashSet<String>> = groups.into_iter().filter(|g| g.len() >= 2).collect();
    groups.sort_by_key(|group| first_seat(group));
    groups
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or(0)
}

fn player_already_in_party(player: &PlayerDisplayData) -> bool {
    player.party_number > 0
}

fn next_party_number(players: &[PlayerDisplayData]) -> i32 {
    players
        .iter()
        .map(|player| player.party_number)
        .max()
        .unwrap_or(0)
        + 1
}

fn assign_party_numbers(
    players: &mut [PlayerDisplayData],
    party_groups: &[HashSet<String>],
    local_puuid: &str,
) {
    let mut party_num = next_party_number(players);
    for group in party_groups {
        if group.contains(local_puuid) {
            continue;
        }

        let mut assigned_any = false;
        for player in players.iter_mut() {
            if group.contains(&player.puuid) && !player_already_in_party(player) {
                player.party_number = party_num;
                player.party_id = format!("party_{party_num}");
                assigned_any = true;
            }
        }
        if assigned_any {
            party_num += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{apply_match_party_ids, assign_party_numbers};
    use crate::riot::types::PlayerDisplayData;
    use std::collections::HashSet;

    #[test]
    fn inferred_party_assignment_skips_groups_containing_local_player() {
        let mut players = vec![
            PlayerDisplayData {
                puuid: "local".into(),
                ..Default::default()
            },
            PlayerDisplayData {
                puuid: "repeat-teammate".into(),
                ..Default::default()
            },
            PlayerDisplayData {
                puuid: "other-party-a".into(),
                ..Default::default()
            },
            PlayerDisplayData {
                puuid: "other-party-b".into(),
                ..Default::default()
            },
        ];
        let party_groups = vec![
            HashSet::from(["local".into(), "repeat-teammate".into()]),
            HashSet::from(["other-party-a".into(), "other-party-b".into()]),
        ];

        assign_party_numbers(&mut players, &party_groups, "local");

        assert_eq!(players[0].party_number, 0);
        assert!(players[0].party_id.is_empty());
        assert_eq!(players[1].party_number, 0);
        assert!(players[1].party_id.is_empty());
        assert_eq!(players[2].party_number, 1);
        assert_eq!(players[2].party_id, "party_1");
        assert_eq!(players[3].party_number, 1);
        assert_eq!(players[3].party_id, "party_1");
    }

    #[test]
    fn inferred_party_assignment_preserves_existing_parties() {
        let mut players = vec![
            PlayerDisplayData {
                puuid: "carried-a".into(),
                party_id: "pregame-party".into(),
                party_number: 1,
                ..Default::default()
            },
            PlayerDisplayData {
                puuid: "carried-b".into(),
                party_id: "pregame-party".into(),
                party_number: 1,
                ..Default::default()
            },
            PlayerDisplayData {
                puuid: "new-a".into(),
                ..Default::default()
            },
            PlayerDisplayData {
                puuid: "new-b".into(),
                ..Default::default()
            },
        ];
        let party_groups = vec![HashSet::from(["new-a".into(), "new-b".into()])];

        assign_party_numbers(&mut players, &party_groups, "local");

        assert_eq!(players[0].party_id, "pregame-party");
        assert_eq!(players[0].party_number, 1);
        assert_eq!(players[1].party_id, "pregame-party");
        assert_eq!(players[1].party_number, 1);
        assert_eq!(players[2].party_number, 2);
        assert_eq!(players[2].party_id, "party_2");
        assert_eq!(players[3].party_number, 2);
        assert_eq!(players[3].party_id, "party_2");
    }

    #[test]
    fn match_party_ids_number_shared_groups_and_clear_solos() {
        let mut players = vec![
            PlayerDisplayData {
                puuid: "stack-a".into(),
                party_id: "uuid-stack".into(),
                ..Default::default()
            },
            PlayerDisplayData {
                puuid: "stack-b".into(),
                party_id: "uuid-stack".into(),
                ..Default::default()
            },
            PlayerDisplayData {
                puuid: "solo".into(),
                party_id: "uuid-solo".into(),
                ..Default::default()
            },
        ];

        apply_match_party_ids(&mut players);

        assert_eq!(players[0].party_number, 1);
        assert_eq!(players[0].party_id, "uuid-stack");
        assert_eq!(players[1].party_number, 1);
        assert_eq!(players[1].party_id, "uuid-stack");
        assert_eq!(players[2].party_number, 0);
        assert!(players[2].party_id.is_empty());
    }
}

#[cfg(test)]
mod detection_tests {
    use super::*;

    const NOW: i64 = 1_000 * HOUR_MS;

    fn entry(match_id: &str, queue_id: &str, hours_ago: i64) -> HistoryEntry {
        HistoryEntry {
            match_id: match_id.into(),
            queue_id: queue_id.into(),
            start_ms: NOW - hours_ago * HOUR_MS,
        }
    }

    fn parties(entries: &[(&str, &str)]) -> MatchParties {
        Arc::new(
            entries
                .iter()
                .map(|(puuid, party)| (puuid.to_string(), party.to_string()))
                .collect(),
        )
    }

    fn pair(shared: &[&str]) -> CandidatePair {
        CandidatePair {
            a: "a".into(),
            b: "b".into(),
            shared: shared.iter().map(|id| id.to_string()).collect(),
        }
    }

    fn player(puuid: &str, team_id: &str) -> PlayerDisplayData {
        PlayerDisplayData {
            puuid: puuid.into(),
            team_id: team_id.into(),
            ..Default::default()
        }
    }

    fn edge(a: &str, b: &str, strength: Strength) -> Edge {
        Edge {
            a: a.into(),
            b: b.into(),
            strength,
        }
    }

    fn history_map(entries: &[(&str, Vec<HistoryEntry>)]) -> HashMap<String, History> {
        entries
            .iter()
            .map(|(puuid, history)| (puuid.to_string(), Arc::new(history.clone())))
            .collect()
    }

    #[test]
    fn shared_lobby_without_shared_party_is_not_linked() {
        // Two swiftplay regulars who keep landing in the same lobbies solo.
        let history = vec![
            entry("m1", "swiftplay", 1),
            entry("m2", "swiftplay", 1),
            entry("m3", "swiftplay", 2),
            entry("m4", "swiftplay", 2),
            entry("m5", "swiftplay", 2),
        ];
        let resolved = HashMap::from([
            ("m1".to_string(), parties(&[("a", "pa"), ("b", "pb")])),
            ("m2".to_string(), parties(&[("a", "pa2"), ("b", "pb2")])),
        ]);

        let strength = link_strength(
            &pair(&["m1", "m2", "m3", "m4", "m5"]),
            &history,
            &history,
            &resolved,
            NOW,
        );

        assert_eq!(strength, None);
    }

    #[test]
    fn partied_in_last_match_is_strong() {
        let history = vec![entry("m1", "competitive", 1), entry("m2", "competitive", 2)];
        let resolved =
            HashMap::from([("m1".to_string(), parties(&[("a", "duo"), ("b", "duo")]))]);

        let strength = link_strength(&pair(&["m1"]), &history, &history, &resolved, NOW);

        assert_eq!(strength, Some(STRONG));
    }

    #[test]
    fn latest_shared_match_apart_overrides_older_party() {
        let history = vec![entry("m1", "unrated", 1), entry("m2", "unrated", 2)];
        let resolved = HashMap::from([
            ("m1".to_string(), parties(&[("a", "solo-a"), ("b", "solo-b")])),
            ("m2".to_string(), parties(&[("a", "duo"), ("b", "duo")])),
        ]);

        let strength = link_strength(&pair(&["m1", "m2"]), &history, &history, &resolved, NOW);

        assert_eq!(strength, None);
    }

    #[test]
    fn deathmatch_warmups_do_not_break_a_party_session() {
        let history_a = vec![
            entry("dm1", "deathmatch", 1),
            entry("dm2", "deathmatch", 1),
            entry("m1", "competitive", 2),
        ];
        let history_b = vec![entry("m1", "competitive", 2)];
        let resolved =
            HashMap::from([("m1".to_string(), parties(&[("a", "duo"), ("b", "duo")]))]);

        let strength = link_strength(&pair(&["m1"]), &history_a, &history_b, &resolved, NOW);

        assert_eq!(strength, Some(STRONG));
    }

    #[test]
    fn stale_or_interrupted_party_is_not_linked() {
        // Partied once, but both have since played several team games apart.
        let history_a = vec![
            entry("x1", "unrated", 1),
            entry("x2", "unrated", 2),
            entry("x3", "unrated", 3),
            entry("m1", "unrated", 4),
        ];
        let resolved =
            HashMap::from([("m1".to_string(), parties(&[("a", "duo"), ("b", "duo")]))]);
        assert_eq!(
            link_strength(&pair(&["m1"]), &history_a, &history_a, &resolved, NOW),
            None
        );

        // Last game together, but a week ago.
        let history_old = vec![entry("m1", "unrated", 24 * 7)];
        assert_eq!(
            link_strength(&pair(&["m1"]), &history_old, &history_old, &resolved, NOW),
            None
        );
    }

    #[test]
    fn repeated_recent_party_survives_a_few_solo_games() {
        let history = vec![
            entry("x1", "unrated", 1),
            entry("x2", "unrated", 2),
            entry("m1", "unrated", 3),
            entry("m2", "unrated", 4),
        ];
        let resolved = HashMap::from([
            ("m1".to_string(), parties(&[("a", "duo"), ("b", "duo")])),
            ("m2".to_string(), parties(&[("a", "duo"), ("b", "duo")])),
        ]);

        let strength = link_strength(&pair(&["m1", "m2"]), &history, &history, &resolved, NOW);

        assert_eq!(strength, Some(WEAK));
    }

    #[test]
    fn unresolved_pairs_fall_back_to_lobby_overlap() {
        let history: Vec<HistoryEntry> = (1..=5)
            .map(|n| entry(&format!("m{n}"), "competitive", n))
            .collect();
        let shared: Vec<&str> = history.iter().map(|e| e.match_id.as_str()).collect();

        let strength = link_strength(&pair(&shared), &history, &history, &HashMap::new(), NOW);

        assert_eq!(strength, Some(WEAK));
    }

    #[test]
    fn candidates_require_same_team_and_ignore_custom_games() {
        let players = vec![
            player("a", "Blue"),
            player("b", "Blue"),
            player("enemy", "Red"),
        ];
        let targets: Vec<&PlayerDisplayData> = players.iter().collect();
        let histories = history_map(&[
            ("a", vec![entry("custom", "", 1), entry("m1", "unrated", 2)]),
            ("b", vec![entry("custom", "", 1), entry("m1", "unrated", 2)]),
            ("enemy", vec![entry("m1", "unrated", 2)]),
        ]);

        let pairs = candidate_pairs(&targets, &histories, false);

        assert_eq!(pairs, vec![pair(&["m1"])]);
    }

    #[test]
    fn free_for_all_candidates_ignore_teams() {
        let players = vec![player("a", "a"), player("b", "b"), player("c", "c")];
        assert!(is_free_for_all_roster(&players));

        let targets: Vec<&PlayerDisplayData> = players.iter().collect();
        let histories = history_map(&[
            ("a", vec![entry("m1", "deathmatch", 1)]),
            ("b", vec![entry("m1", "deathmatch", 1)]),
        ]);

        assert_eq!(candidate_pairs(&targets, &histories, true).len(), 1);
    }

    #[test]
    fn queue_rules_follow_the_mode() {
        assert!(queue_rules("", "CustomGame").skip);
        assert!(queue_rules("premier", "Matchmaking").skip);
        assert!(queue_rules("competitive", "Matchmaking").no_four_stacks);
        assert!(!queue_rules("swiftplay", "Matchmaking").no_four_stacks);
        assert!(queue_rules("deathmatch", "Matchmaking").free_for_all);
        assert!(!queue_rules("hurm", "Matchmaking").free_for_all);
    }

    #[test]
    fn competitive_never_forms_a_four_stack() {
        let rules = queue_rules("competitive", "Matchmaking");
        let edges = vec![
            edge("a", "b", STRONG),
            edge("c", "d", STRONG),
            // A weak bridge would turn two duos into an illegal four-stack.
            edge("b", "c", WEAK),
        ];

        let groups = cluster(edges, rules, &HashMap::new());

        assert_eq!(groups.len(), 2);
        assert!(groups.iter().all(|group| group.len() == 2));
    }

    #[test]
    fn competitive_five_stack_still_forms() {
        let rules = queue_rules("competitive", "Matchmaking");
        let members = ["a", "b", "c", "d", "e"];
        let mut edges = Vec::new();
        for (i, x) in members.iter().enumerate() {
            for y in &members[i + 1..] {
                edges.push(edge(x, y, STRONG));
            }
        }

        let groups = cluster(edges, rules, &HashMap::new());

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].len(), 5);
    }

    #[test]
    fn unrated_allows_four_but_caps_at_five() {
        let rules = queue_rules("unrated", "Matchmaking");
        let edges = vec![
            edge("a", "b", STRONG),
            edge("b", "c", STRONG),
            edge("c", "d", STRONG),
            edge("d", "e", LIKELY),
            edge("e", "f", WEAK),
        ];

        let groups = cluster(edges, rules, &HashMap::new());

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].len(), 5);
        assert!(!groups[0].contains("f"));
    }

    #[test]
    fn resolves_each_pairs_newest_shared_match_first() {
        let pairs = vec![
            CandidatePair {
                a: "a".into(),
                b: "b".into(),
                shared: vec!["m1".into(), "m2".into()],
            },
            CandidatePair {
                a: "c".into(),
                b: "d".into(),
                shared: vec!["m9".into(), "m1".into()],
            },
        ];

        assert_eq!(matches_to_resolve(&pairs, 2), vec!["m1", "m9"]);
    }

    #[test]
    fn parses_history_queue_and_start_time() {
        let value = serde_json::json!({
            "History": [
                {"MatchID": "m1", "GameStartTime": 123, "QueueID": "Competitive"},
                {"MatchID": "", "GameStartTime": 1, "QueueID": "unrated"}
            ]
        });

        assert_eq!(
            parse_history(&value),
            vec![HistoryEntry {
                match_id: "m1".into(),
                queue_id: "competitive".into(),
                start_ms: 123,
            }]
        );
    }
}
