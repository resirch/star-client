use crate::riot::types::{rank_name, MmrResponse};

pub struct RankInfo {
    pub tier: i32,
    pub name: String,
    pub rr: i32,
    pub peak_tier: i32,
    pub peak_name: String,
    pub leaderboard_pos: i32,
}

pub fn extract_rank(mmr: &MmrResponse, season_id: Option<&str>) -> RankInfo {
    let mut info = RankInfo {
        tier: 0,
        name: "Unranked".into(),
        rr: 0,
        peak_tier: 0,
        peak_name: "Unranked".into(),
        leaderboard_pos: 0,
    };

    if let Some(skills) = &mmr.queue_skills {
        if let Some(comp) = skills.get("competitive") {
            if let Some(seasonal) = &comp.seasonal_info_by_season_i_d {
                if let Some(sid) = season_id {
                    if let Some(s) = seasonal.get(sid) {
                        info.tier = s.competitive_tier.unwrap_or(0);
                        info.name = rank_name(info.tier).to_string();
                        info.rr = s.ranking_in_tier.unwrap_or(0);
                        info.leaderboard_pos = s.leaderboard_rank.unwrap_or(0);
                    }
                }

                for s_info in seasonal.values() {
                    let tier = s_info.competitive_tier.unwrap_or(0);
                    if tier > info.peak_tier {
                        info.peak_tier = tier;
                    }
                    if let Some(wins_by_tier) = &s_info.wins_by_tier {
                        for (t_str, _) in wins_by_tier {
                            if let Ok(t) = t_str.parse::<i32>() {
                                if t > info.peak_tier {
                                    info.peak_tier = t;
                                }
                            }
                        }
                    }
                }
                info.peak_name = rank_name(info.peak_tier).to_string();
            }
        }
    }

    info
}
