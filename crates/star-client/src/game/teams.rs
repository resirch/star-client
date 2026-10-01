use crate::game::match_data::MatchContext;
use crate::riot::types::PlayerDisplayData;
use std::collections::HashMap;

pub const MIN_DUO_PARTY_GROUPS_FOR_MULTI_TEAM: usize = 4;

pub fn is_standard_team_id(team_id: &str) -> bool {
    team_id.eq_ignore_ascii_case("blue") || team_id.eq_ignore_ascii_case("red")
}

pub fn is_gauntlet_context(ctx: &MatchContext) -> bool {
    let queue = ctx.queue.to_ascii_lowercase();
    let mode = ctx.mode.to_ascii_lowercase();
    let map = ctx.map.name.to_ascii_lowercase();
    queue.contains("gauntlet")
        || mode.contains("gauntlet")
        || map.contains("gauntlet")
        || queue.contains("gglitch")
        || mode.contains("gglitch")
}

fn roster_party_id(player: &PlayerDisplayData) -> &str {
    if !player.match_party_id.is_empty() {
        player.match_party_id.as_str()
    } else {
        player.party_id.as_str()
    }
}

pub fn duo_party_group_count_for_roster(players: &[PlayerDisplayData]) -> usize {
    duo_party_group_count(players)
}

fn duo_party_group_count(players: &[PlayerDisplayData]) -> usize {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for player in players {
        let party_id = roster_party_id(player);
        if !party_id.is_empty() {
            *counts.entry(party_id).or_insert(0) += 1;
        }
    }
    counts.values().filter(|count| **count == 2).count()
}

fn duo_roster_team_count(players: &[PlayerDisplayData]) -> usize {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for player in players {
        if !player.roster_team_id.is_empty() {
            *counts.entry(player.roster_team_id.as_str()).or_insert(0) += 1;
        }
    }
    counts.values().filter(|count| **count == 2).count()
}

pub fn is_large_standard_team_lobby(players: &[PlayerDisplayData]) -> bool {
    if players.len() < 10 {
        return false;
    }

    let team_ids = unique_team_ids(players);
    if team_ids.len() > 2 {
        return false;
    }
    if !team_ids.iter().all(|team_id| is_standard_team_id(team_id)) {
        return false;
    }

    let largest_faction = team_ids
        .iter()
        .map(|team_id| {
            players
                .iter()
                .filter(|player| player.team_id.eq_ignore_ascii_case(team_id))
                .count()
        })
        .max()
        .unwrap_or(0);

    largest_faction > 4
}

pub fn should_group_by_roster_team(players: &[PlayerDisplayData], ctx: Option<&MatchContext>) -> bool {
    let duo_teams = duo_roster_team_count(players);
    if duo_teams >= MIN_DUO_PARTY_GROUPS_FOR_MULTI_TEAM {
        return true;
    }
    if ctx.is_some_and(is_gauntlet_context) && duo_teams >= 1 {
        return true;
    }
    if is_large_standard_team_lobby(players) && duo_teams >= 1 {
        return true;
    }
    false
}

pub fn should_group_by_party(players: &[PlayerDisplayData], ctx: Option<&MatchContext>) -> bool {
    let duo_groups = duo_party_group_count(players);
    if duo_groups >= MIN_DUO_PARTY_GROUPS_FOR_MULTI_TEAM {
        return true;
    }
    if ctx.is_some_and(is_gauntlet_context) && duo_groups >= 1 {
        return true;
    }
    false
}

fn unique_team_ids(players: &[PlayerDisplayData]) -> Vec<String> {
    let mut ids: Vec<String> = players
        .iter()
        .map(|player| player.team_id.clone())
        .filter(|team_id| !team_id.is_empty())
        .collect();
    ids.sort();
    ids.dedup();
    ids
}

pub fn uses_multi_team_layout(players: &[PlayerDisplayData], ctx: Option<&MatchContext>) -> bool {
    if ctx.is_some_and(is_gauntlet_context) {
        return true;
    }

    if is_large_standard_team_lobby(players) {
        return true;
    }

    if should_group_by_roster_team(players, ctx) {
        return true;
    }

    if should_group_by_party(players, ctx) {
        return true;
    }

    let teams = unique_team_ids(players);
    if teams.len() > 2 {
        return true;
    }

    teams
        .iter()
        .any(|team_id| !team_id.is_empty() && !is_standard_team_id(team_id))
}

fn explicit_duo_group_key(
    player: &PlayerDisplayData,
    players: &[PlayerDisplayData],
    group_by_party: bool,
) -> Option<String> {
    if !player.roster_team_id.is_empty() {
        let count = players
            .iter()
            .filter(|candidate| candidate.roster_team_id == player.roster_team_id)
            .count();
        if count == 2 {
            return Some(format!("roster:{}", player.roster_team_id));
        }
    }

    if group_by_party {
        let party_id = roster_party_id(player);
        if !party_id.is_empty() {
            let count = players
                .iter()
                .filter(|candidate| roster_party_id(candidate) == party_id)
                .count();
            if count == 2 {
                return Some(format!("party:{}", party_id));
            }
        }
    }

    if player.party_number > 0 {
        let count = players
            .iter()
            .filter(|candidate| candidate.party_number == player.party_number)
            .count();
        if count == 2 {
            return Some(format!("pnum:{}", player.party_number));
        }
    }

    if !player.team_id.is_empty()
        && !is_standard_team_id(&player.team_id)
        && !is_large_standard_team_lobby(players)
    {
        let count = players
            .iter()
            .filter(|candidate| candidate.team_id == player.team_id)
            .count();
        if count == 2 {
            return Some(format!("team:{}", player.team_id));
        }
    }

    if !player.team_id.is_empty() && !is_large_standard_team_lobby(players) {
        let team_count = unique_team_ids(players).len();
        if team_count <= 2 {
            return Some(format!("team:{}", player.team_id));
        }
    }

    None
}

pub fn build_team_group_keys(
    players: &[PlayerDisplayData],
    group_by_party: bool,
) -> HashMap<String, String> {
    let mut keys: HashMap<String, String> = HashMap::new();
    for player in players {
        keys.entry(player.puuid.clone()).or_insert_with(|| {
            explicit_duo_group_key(player, players, group_by_party)
                .unwrap_or_else(|| format!("solo:{}", player.puuid))
        });
    }

    keys
}

pub fn team_group_key(
    player: &PlayerDisplayData,
    players: &[PlayerDisplayData],
    group_by_party: bool,
) -> String {
    build_team_group_keys(players, group_by_party)
        .get(&player.puuid)
        .cloned()
        .unwrap_or_else(|| format!("solo:{}", player.puuid))
}

pub fn roster_uses_multi_team_layout(
    players: &[PlayerDisplayData],
    ctx: Option<&MatchContext>,
) -> bool {
    uses_multi_team_layout(players, ctx)
}

pub struct TeamSection<'a> {
    pub team_id: &'a str,
    pub heading: String,
    pub is_local: bool,
    pub players: Vec<&'a PlayerDisplayData>,
}

pub const TEAM_GROUPING_UNAVAILABLE_MESSAGE: &str =
    "Could not resolve teams for this mode — showing all players.";

pub struct TeamLayout<'a> {
    pub sections: Vec<TeamSection<'a>>,
    pub notice: Option<&'static str>,
}

pub fn has_reliable_duo_grouping(
    players: &[PlayerDisplayData],
    ctx: Option<&MatchContext>,
) -> bool {
    if !uses_multi_team_layout(players, ctx) {
        return true;
    }

    duo_roster_team_count(players) >= MIN_DUO_PARTY_GROUPS_FOR_MULTI_TEAM
        || duo_party_group_count(players) >= MIN_DUO_PARTY_GROUPS_FOR_MULTI_TEAM
}

pub fn team_layout<'a>(
    players: &'a [PlayerDisplayData],
    local_puuid: &str,
    ctx: Option<&MatchContext>,
    ingame: bool,
) -> TeamLayout<'a> {
    if uses_multi_team_layout(players, ctx) && !has_reliable_duo_grouping(players, ctx) {
        return TeamLayout {
            sections: free_for_all_sections(players),
            notice: if ingame {
                Some(TEAM_GROUPING_UNAVAILABLE_MESSAGE)
            } else {
                None
            },
        };
    }

    let sections = if uses_multi_team_layout(players, ctx) {
        let group_by_party = should_group_by_party(players, ctx);
        multi_team_sections(players, local_puuid, group_by_party)
    } else {
        classic_team_sections(players, local_puuid)
    };

    TeamLayout {
        sections,
        notice: None,
    }
}

pub fn team_sections<'a>(
    players: &'a [PlayerDisplayData],
    local_puuid: &str,
    ctx: Option<&MatchContext>,
) -> Vec<TeamSection<'a>> {
    team_layout(players, local_puuid, ctx, false).sections
}

fn free_for_all_sections<'a>(players: &'a [PlayerDisplayData]) -> Vec<TeamSection<'a>> {
    if players.is_empty() {
        return Vec::new();
    }

    vec![TeamSection {
        team_id: players[0].team_id.as_str(),
        heading: "PLAYERS".to_string(),
        is_local: true,
        players: players.iter().collect(),
    }]
}

fn classic_team_sections<'a>(
    players: &'a [PlayerDisplayData],
    local_puuid: &str,
) -> Vec<TeamSection<'a>> {
    let my_team = players
        .iter()
        .find(|player| player.puuid == local_puuid)
        .map(|player| player.team_id.as_str())
        .unwrap_or_default();
    let (allies, enemies): (Vec<_>, Vec<_>) = players
        .iter()
        .partition(|player| player.team_id == my_team || my_team.is_empty());

    let mut sections = Vec::new();
    if !allies.is_empty() {
        sections.push(TeamSection {
            team_id: allies[0].team_id.as_str(),
            heading: "YOUR TEAM".to_string(),
            is_local: true,
            players: allies,
        });
    }
    if !enemies.is_empty() {
        sections.push(TeamSection {
            team_id: enemies[0].team_id.as_str(),
            heading: "ENEMY TEAM".to_string(),
            is_local: false,
            players: enemies,
        });
    }
    sections
}

fn multi_team_sections<'a>(
    players: &'a [PlayerDisplayData],
    local_puuid: &str,
    group_by_party: bool,
) -> Vec<TeamSection<'a>> {
    let group_keys = build_team_group_keys(players, group_by_party);
    let my_key = players
        .iter()
        .find(|player| player.puuid == local_puuid)
        .and_then(|player| group_keys.get(&player.puuid))
        .cloned()
        .unwrap_or_default();
    let mut grouped: HashMap<String, Vec<&PlayerDisplayData>> = HashMap::new();

    for player in players {
        let key = group_keys
            .get(&player.puuid)
            .cloned()
            .unwrap_or_else(|| format!("solo:{}", player.puuid));
        grouped.entry(key).or_default().push(player);
    }

    let mut group_keys: Vec<String> = grouped.keys().cloned().collect();
    group_keys.sort();
    if !my_key.is_empty() {
        group_keys.sort_by_key(|key| (*key != my_key, key.clone()));
    }

    let mut opponent_index = 0usize;
    group_keys
        .into_iter()
        .filter_map(|key| {
            let team_players = grouped.remove(&key)?;
            if team_players.is_empty() {
                return None;
            }

            let is_local = !my_key.is_empty() && key == my_key;
            let heading = if is_local {
                "YOUR TEAM".to_string()
            } else {
                opponent_index += 1;
                format!("OPPONENT {opponent_index}")
            };

            let team_id = team_players[0].team_id.as_str();
            Some(TeamSection {
                team_id,
                heading,
                is_local,
                players: team_players,
            })
        })
        .collect()
}

/// Assigns stable O/X markers for anonymous duos (e.g. Gauntlet teams of two).
pub fn apply_incognito_team_markers(players: &mut [PlayerDisplayData]) {
    let group_by_party = should_group_by_party(players, None);
    let mut by_group: HashMap<String, Vec<usize>> = HashMap::new();
    for (index, player) in players.iter().enumerate() {
        let key = team_group_key(player, players, group_by_party);
        if key.is_empty() {
            continue;
        }
        by_group.entry(key).or_default().push(index);
    }

    let mut markers = Vec::new();
    for indices in by_group.values() {
        if indices.len() != 2 {
            continue;
        }

        let mut sorted = indices.clone();
        sorted.sort_by_key(|index| players[*index].puuid.clone());
        markers.push((sorted[0], 'O'));
        markers.push((sorted[1], 'X'));
    }

    for (index, marker) in markers {
        players[index].incognito_team_marker = Some(marker);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::match_data::{MapInfo, MatchContext};

    fn player(puuid: &str, team_id: &str) -> PlayerDisplayData {
        PlayerDisplayData {
            puuid: puuid.into(),
            team_id: team_id.into(),
            ..Default::default()
        }
    }

    fn duo_roster(
        puuid: &str,
        team_id: &str,
        roster_team_id: &str,
    ) -> PlayerDisplayData {
        PlayerDisplayData {
            puuid: puuid.into(),
            team_id: team_id.into(),
            roster_team_id: roster_team_id.into(),
            ..Default::default()
        }
    }

    fn gauntlet_context() -> MatchContext {
        MatchContext {
            map: MapInfo {
                name: "Gauntlet".into(),
            },
            queue: "hurmgauntlet".into(),
            mode: "gauntlet".into(),
            server_id: String::new(),
        }
    }

    #[test]
    fn detects_large_blue_red_lobby_as_multi_team() {
        let mut players = Vec::new();
        for i in 0..14 {
            players.push(PlayerDisplayData {
                puuid: format!("ally-{i}"),
                team_id: "Blue".into(),
                ..Default::default()
            });
        }
        for i in 0..2 {
            players.push(PlayerDisplayData {
                puuid: format!("enemy-{i}"),
                team_id: "Red".into(),
                ..Default::default()
            });
        }

        assert!(is_large_standard_team_lobby(&players));
        assert!(uses_multi_team_layout(&players, None));
    }

    #[test]
    fn gauntlet_blue_red_split_uses_roster_team_groups() {
        let players = vec![
            duo_roster("self", "Blue", "roster-1"),
            duo_roster("mate", "Blue", "roster-1"),
            duo_roster("o1a", "Blue", "roster-2"),
            duo_roster("o1b", "Blue", "roster-2"),
            duo_roster("o2a", "Blue", "roster-3"),
            duo_roster("o2b", "Blue", "roster-3"),
            duo_roster("o3a", "Blue", "roster-4"),
            duo_roster("o3b", "Blue", "roster-4"),
            duo_roster("o4a", "Red", "roster-5"),
            duo_roster("o4b", "Red", "roster-5"),
            duo_roster("o5a", "Blue", "roster-6"),
            duo_roster("o5b", "Blue", "roster-6"),
            duo_roster("o6a", "Blue", "roster-7"),
            duo_roster("o6b", "Blue", "roster-7"),
            duo_roster("o7a", "Blue", "roster-8"),
            duo_roster("o7b", "Blue", "roster-8"),
        ];

        let sections = team_sections(&players, "self", None);
        assert_eq!(sections.len(), 8);
        assert_eq!(sections[0].heading, "YOUR TEAM");
        assert_eq!(sections[0].players.len(), 2);
    }

    #[test]
    fn shared_roster_team_ids_form_pairs() {
        let players = vec![
            duo_roster("self", "Blue", "team-1"),
            duo_roster("mate", "Blue", "team-1"),
            duo_roster("o1a", "Blue", "team-2"),
            duo_roster("o1b", "Blue", "team-2"),
        ];

        let keys = build_team_group_keys(&players, false);
        assert_eq!(keys.get("self"), keys.get("mate"));
        assert_eq!(keys.get("o1a"), keys.get("o1b"));
        assert_ne!(keys.get("self"), keys.get("o1a"));
    }

    #[test]
    fn ingame_without_duo_data_uses_free_for_all_with_notice() {
        let mut players = Vec::new();
        for i in 0..16 {
            players.push(PlayerDisplayData {
                puuid: format!("p{i:02}"),
                team_id: "Blue".into(),
                ..Default::default()
            });
        }

        let layout = team_layout(&players, "p00", None, true);
        assert_eq!(layout.sections.len(), 1);
        assert_eq!(layout.sections[0].heading, "PLAYERS");
        assert_eq!(layout.sections[0].players.len(), 16);
        assert_eq!(
            layout.notice,
            Some(TEAM_GROUPING_UNAVAILABLE_MESSAGE)
        );
    }

    fn does_not_invent_duo_pairs_in_large_lobby_without_roster() {
        let mut players = Vec::new();
        for i in 0..12 {
            players.push(PlayerDisplayData {
                puuid: format!("p{i:02}"),
                team_id: "Blue".into(),
                ..Default::default()
            });
        }
        for i in 0..2 {
            players.push(PlayerDisplayData {
                puuid: format!("r{i}"),
                team_id: "Red".into(),
                ..Default::default()
            });
        }

        let keys = build_team_group_keys(&players, false);
        let unique_keys: std::collections::HashSet<_> = keys.values().cloned().collect();
        assert_eq!(unique_keys.len(), 14);
        assert!(!keys.values().any(|key| key.starts_with("pair:")));
    }

    #[test]
    fn classic_layout_for_red_and_blue() {
        let players = vec![
            player("a", "Blue"),
            player("b", "Red"),
            player("c", "Blue"),
        ];

        assert!(!uses_multi_team_layout(&players, None));
    }
}
