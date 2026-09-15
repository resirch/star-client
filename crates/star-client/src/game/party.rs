use crate::riot::api::RiotApiClient;
use crate::riot::types::PlayerDisplayData;
use std::collections::{HashMap, HashSet};

const PARTY_MATCH_THRESHOLD: usize = 4;
const HISTORY_COUNT: usize = 5;

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

/// Detects non-local parties by comparing recent match histories.
/// Players who shared >= 4 of their last 5 matches are likely in the same party.
/// Players who already have a party assignment are left untouched.
/// The local player's party is not inferred here; Riot presence data is
/// authoritative for that because it exposes the actual local party ID.
pub async fn detect_parties(api: &RiotApiClient, players: &mut [PlayerDisplayData]) {
    let local_puuid = api.puuid();
    let targets: Vec<String> = players
        .iter()
        .filter(|player| player.puuid != local_puuid && !player_already_in_party(player))
        .map(|player| player.puuid.clone())
        .collect();

    let results =
        futures_util::future::join_all(targets.iter().map(|puuid| api.get_match_history(puuid)))
            .await;

    let mut match_histories: HashMap<String, Vec<String>> = HashMap::new();
    for (puuid, result) in targets.iter().zip(results) {
        if let Ok(history) = result {
            let match_ids: Vec<String> = history["History"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .take(HISTORY_COUNT)
                        .filter_map(|m| m["MatchID"].as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            match_histories.insert(puuid.clone(), match_ids);
        }
    }

    // Compare histories to find party groups
    let mut party_groups: Vec<HashSet<String>> = Vec::new();

    let puuids: Vec<String> = match_histories.keys().cloned().collect();
    for i in 0..puuids.len() {
        for j in (i + 1)..puuids.len() {
            let a = &puuids[i];
            let b = &puuids[j];
            if let (Some(hist_a), Some(hist_b)) = (match_histories.get(a), match_histories.get(b)) {
                let shared: usize = hist_a.iter().filter(|m| hist_b.contains(m)).count();
                if shared >= PARTY_MATCH_THRESHOLD {
                    let mut found_group = false;
                    for group in &mut party_groups {
                        if group.contains(a) || group.contains(b) {
                            group.insert(a.clone());
                            group.insert(b.clone());
                            found_group = true;
                            break;
                        }
                    }
                    if !found_group {
                        let mut new_group = HashSet::new();
                        new_group.insert(a.clone());
                        new_group.insert(b.clone());
                        party_groups.push(new_group);
                    }
                }
            }
        }
    }

    assign_party_numbers(players, &party_groups, local_puuid);
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
