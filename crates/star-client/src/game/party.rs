use crate::riot::api::RiotApiClient;
use crate::riot::types::PlayerDisplayData;
use std::collections::{HashMap, HashSet};

const PARTY_MATCH_THRESHOLD: usize = 4;
const HISTORY_COUNT: usize = 5;

/// Detects non-local parties by comparing recent match histories.
/// Players who shared >= 4 of their last 5 matches are likely in the same party.
/// The local player's party is not inferred here; Riot presence data is
/// authoritative for that because it exposes the actual local party ID.
pub async fn detect_parties(api: &RiotApiClient, players: &mut [PlayerDisplayData]) {
    let mut match_histories: HashMap<String, Vec<String>> = HashMap::new();
    let local_puuid = api.puuid();

    // Fetch recent match IDs for each player
    for player in players.iter() {
        if player.puuid == local_puuid {
            continue;
        }

        if let Ok(history) = api.get_match_history(&player.puuid).await {
            let match_ids: Vec<String> = history["History"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .take(HISTORY_COUNT)
                        .filter_map(|m| m["MatchID"].as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            match_histories.insert(player.puuid.clone(), match_ids);
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

fn assign_party_numbers(
    players: &mut [PlayerDisplayData],
    party_groups: &[HashSet<String>],
    local_puuid: &str,
) {
    let mut party_num = 1;
    for group in party_groups {
        if group.contains(local_puuid) {
            continue;
        }

        for player in players.iter_mut() {
            if group.contains(&player.puuid) {
                player.party_number = party_num;
                player.party_id = format!("party_{party_num}");
            }
        }
        party_num += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::assign_party_numbers;
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
}
