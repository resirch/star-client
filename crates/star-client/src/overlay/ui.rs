use crate::config::{ColumnConfig, Config};
use crate::game::state::GameState;
use crate::overlay::theme;
use crate::riot::types::{rank_color, PlayerDisplayData};
use egui::text::{LayoutJob, TextFormat};
use egui::{Align, Align2, Layout, Pos2, Rect, RichText, Ui, Vec2};

const PARTY_W: f32 = 8.0;
const STAR_W: f32 = 18.0;
const AGENT_W: f32 = 62.0;
const NAME_W: f32 = 125.0;
const RANK_W: f32 = 82.0;
const RR_W: f32 = 55.0;
const PEAK_W: f32 = 82.0;
const PREV_W: f32 = 82.0;
const LB_W: f32 = 40.0;
const KD_W: f32 = 48.0;
const HS_W: f32 = 48.0;
const WR_W: f32 = 50.0;
const ERR_W: f32 = 74.0;
const LVL_W: f32 = 38.0;
const SKIN_W: f32 = 156.0;
const ROW_H: f32 = 22.0;
const HDR_H: f32 = 20.0;
const CELL_PAD: f32 = 3.0;

pub fn render_overlay(
    ctx: &egui::Context,
    game_state: &GameState,
    players: &[PlayerDisplayData],
    local_puuid: &str,
    config: &Config,
) {
    if players.is_empty() {
        return;
    }

    let columns = &config.columns;
    let screen = ctx.screen_rect();
    let show_leaderboard = leaderboard_column_visible(columns, players);
    let show_skin = skin_column_visible(columns, game_state);
    let tw = table_width(columns, show_leaderboard, show_skin);
    let x = (screen.width() - tw) / 2.0;
    let y = 60.0;

    egui::Area::new(egui::Id::new("star_overlay"))
        .fixed_pos(Pos2::new(x, y))
        .order(egui::Order::Foreground)
        .interactable(false)
        .show(ctx, |ui| {
            egui::Frame::none()
                .fill(theme::BG_COLOR)
                .rounding(theme::table_rounding())
                .stroke(theme::table_stroke())
                .inner_margin(6.0)
                .show(ui, |ui: &mut Ui| {
                    ui.set_min_width(tw);
                    title_bar(ui, game_state);
                    ui.add_space(4.0);
                    header_row(
                        ui,
                        columns,
                        show_leaderboard,
                        show_skin,
                        &selected_weapon_label(&config.overlay.weapon),
                    );
                    ui.add_space(2.0);

                    let (allies, enemies) = split_players_by_team(players, local_puuid);

                    if !allies.is_empty() {
                        team_label(ui, "YOUR TEAM");
                        for p in &allies {
                            player_row(ui, p, config, true, show_leaderboard, show_skin);
                        }
                    }

                    if !enemies.is_empty() {
                        ui.add_space(6.0);
                        team_label(ui, "ENEMY TEAM");
                        for p in &enemies {
                            player_row(ui, p, config, false, show_leaderboard, show_skin);
                        }
                    }
                });
        });
}

fn split_players_by_team<'a>(
    players: &'a [PlayerDisplayData],
    local_puuid: &str,
) -> (Vec<&'a PlayerDisplayData>, Vec<&'a PlayerDisplayData>) {
    let my_team = players
        .iter()
        .find(|player| player.puuid == local_puuid)
        .or_else(|| players.first())
        .map(|player| player.team_id.as_str())
        .unwrap_or("");

    players
        .iter()
        .partition(|player| player.team_id == my_team || my_team.is_empty())
}

fn leaderboard_column_visible(c: &ColumnConfig, players: &[PlayerDisplayData]) -> bool {
    c.leaderboard && players.iter().any(|p| p.leaderboard_position > 0)
}

fn skin_column_visible(c: &ColumnConfig, state: &GameState) -> bool {
    c.skin && matches!(state, GameState::Ingame { .. })
}

fn table_width(c: &ColumnConfig, show_leaderboard: bool, show_skin: bool) -> f32 {
    let mut w = PARTY_W + STAR_W + AGENT_W + NAME_W + RANK_W;
    if c.rr {
        w += RR_W;
    }
    if c.peak_rank {
        w += PEAK_W;
    }
    if c.previous_rank {
        w += PREV_W;
    }
    if show_leaderboard {
        w += LB_W;
    }
    if c.kd {
        w += KD_W;
    }
    if c.headshot_percent {
        w += HS_W;
    }
    if c.winrate {
        w += WR_W;
    }
    if c.earned_rr {
        w += ERR_W;
    }
    if c.level {
        w += LVL_W;
    }
    if show_skin {
        w += SKIN_W;
    }
    w + 12.0
}

fn title_bar(ui: &mut Ui, state: &GameState) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("★ STAR CLIENT")
                .font(theme::header_font())
                .color(theme::STAR_COLOR),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(
                RichText::new(state.to_string())
                    .font(theme::small_font())
                    .color(state_color(state)),
            );
        });
    });
}

fn header_row(
    ui: &mut Ui,
    c: &ColumnConfig,
    show_leaderboard: bool,
    show_skin: bool,
    skin_label: &str,
) {
    let origin = ui.cursor().min;
    let full_w = ui.available_width();
    ui.painter().rect_filled(
        Rect::from_min_size(origin, Vec2::new(full_w, HDR_H)),
        2.0,
        theme::HEADER_BG,
    );

    ui.horizontal(|ui| {
        ui.set_height(HDR_H);
        let f = theme::small_font();
        let clr = theme::TEXT_MUTED;

        hdr_cell(ui, "", PARTY_W, &f, clr);
        hdr_cell(ui, "", STAR_W, &f, clr);
        hdr_cell(ui, "AGENT", AGENT_W, &f, clr);
        hdr_cell(ui, "NAME", NAME_W, &f, clr);
        hdr_cell(ui, "RANK", RANK_W, &f, clr);
        if c.rr {
            hdr_cell(ui, "RR", RR_W, &f, clr);
        }
        if c.peak_rank {
            hdr_cell(ui, "PEAK", PEAK_W, &f, clr);
        }
        if c.previous_rank {
            hdr_cell(ui, "PREV", PREV_W, &f, clr);
        }
        if show_leaderboard {
            hdr_cell(ui, "#", LB_W, &f, clr);
        }
        if c.kd {
            hdr_cell(ui, "K/D", KD_W, &f, clr);
        }
        if c.headshot_percent {
            hdr_cell(ui, "HS%", HS_W, &f, clr);
        }
        if c.winrate {
            hdr_cell(ui, "WR%", WR_W, &f, clr);
        }
        if c.earned_rr {
            hdr_cell(ui, "ΔRR", ERR_W, &f, clr);
        }
        if c.level {
            hdr_cell(ui, "LVL", LVL_W, &f, clr);
        }
        if show_skin {
            hdr_cell(ui, skin_label, SKIN_W, &f, clr);
        }
    });
}

fn hdr_cell(ui: &mut Ui, text: &str, w: f32, font: &egui::FontId, color: egui::Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, HDR_H), egui::Sense::hover());
    if !text.is_empty() {
        ui.painter().text(
            Pos2::new(rect.left() + CELL_PAD, rect.center().y),
            Align2::LEFT_CENTER,
            text,
            font.clone(),
            color,
        );
    }
}

fn team_label(ui: &mut Ui, text: &str) {
    ui.add_space(2.0);
    let is_ally = text.eq_ignore_ascii_case("YOUR TEAM");
    ui.label(
        RichText::new(text)
            .font(theme::small_font())
            .color(theme::team_text_color(is_ally)),
    );
    ui.add_space(1.0);
}

fn player_row(
    ui: &mut Ui,
    p: &PlayerDisplayData,
    config: &Config,
    is_ally: bool,
    show_leaderboard: bool,
    show_skin: bool,
) {
    let c = &config.columns;
    let bg = if is_ally {
        theme::ROW_BG_ALLY
    } else {
        theme::ROW_BG_ENEMY
    };
    let origin = ui.cursor().min;
    let full_w = ui.available_width();
    ui.painter().rect_filled(
        Rect::from_min_size(origin, Vec2::new(full_w, ROW_H)),
        2.0,
        bg,
    );

    ui.horizontal(|ui| {
        ui.set_height(ROW_H);
        let f = theme::body_font();
        let loading = loading_dots(ui.ctx());

        // Party bar
        let (rect, _) = ui.allocate_exact_size(Vec2::new(PARTY_W, ROW_H), egui::Sense::hover());
        if p.party_number > 0 {
            let bar = Rect::from_center_size(rect.center(), Vec2::new(4.0, ROW_H - 4.0));
            ui.painter()
                .rect_filled(bar, 2.0, theme::party_color(p.party_number));
        }

        // Star
        let (rect, _) = ui.allocate_exact_size(Vec2::new(STAR_W, ROW_H), egui::Sense::hover());
        if p.is_star_user {
            ui.painter().text(
                rect.center(),
                Align2::CENTER_CENTER,
                "★",
                theme::star_font(),
                theme::STAR_COLOR,
            );
        }

        // Agent
        text_cell(
            ui,
            &p.agent_name,
            AGENT_W,
            &f,
            theme::agent_color(&p.agent_name),
        );

        // Name
        let name = if p.is_incognito {
            "---".into()
        } else {
            format!("{}#{}", p.game_name, p.tag_line)
        };
        text_cell(ui, &name, NAME_W, &f, theme::team_text_color(is_ally));

        // Rank (always shown)
        if p.enriched {
            text_cell(ui, &p.rank_name, RANK_W, &f, rank_color(p.current_rank));
        } else {
            text_cell(ui, &loading, RANK_W, &f, theme::TEXT_MUTED);
        }

        if c.rr {
            let t = if p.enriched {
                if p.current_rank > 0 {
                    format!("{} RR", p.rr)
                } else {
                    "-".into()
                }
            } else {
                loading.clone()
            };
            text_cell(ui, &t, RR_W, &f, theme::TEXT_SECONDARY);
        }

        if c.peak_rank {
            if p.enriched {
                let t = if p.peak_rank > 0 {
                    &p.peak_rank_name
                } else {
                    "-"
                };
                text_cell(ui, t, PEAK_W, &f, rank_color(p.peak_rank));
            } else {
                text_cell(ui, &loading, PEAK_W, &f, theme::TEXT_MUTED);
            }
        }

        if c.previous_rank {
            if p.enriched {
                let t = if p.previous_rank > 0 {
                    &p.previous_rank_name
                } else {
                    "-"
                };
                text_cell(ui, t, PREV_W, &f, rank_color(p.previous_rank));
            } else {
                text_cell(ui, &loading, PREV_W, &f, theme::TEXT_MUTED);
            }
        }

        if show_leaderboard {
            let t = if p.enriched {
                if p.leaderboard_position > 0 {
                    format!("#{}", p.leaderboard_position)
                } else {
                    "-".into()
                }
            } else {
                loading.clone()
            };
            text_cell(ui, &t, LB_W, &f, theme::TEXT_SECONDARY);
        }

        if c.kd {
            let t = if p.enriched {
                if p.kd > 0.0 {
                    format!("{:.2}", p.kd)
                } else {
                    "-".into()
                }
            } else {
                loading.clone()
            };
            let clr = if p.enriched {
                theme::kd_color(p.kd)
            } else {
                theme::TEXT_MUTED
            };
            text_cell(ui, &t, KD_W, &f, clr);
        }

        if c.headshot_percent {
            let t = if p.enriched {
                if p.headshot_percent > 0.0 {
                    format!("{:.0}%", p.headshot_percent)
                } else {
                    "-".into()
                }
            } else {
                loading.clone()
            };
            let clr = if p.enriched {
                theme::hs_color(p.headshot_percent)
            } else {
                theme::TEXT_MUTED
            };
            text_cell(ui, &t, HS_W, &f, clr);
        }

        if c.winrate {
            let t = if p.enriched {
                if p.games > 0 {
                    format!("{:.0}%", p.winrate)
                } else {
                    "-".into()
                }
            } else {
                loading.clone()
            };
            let clr = if p.enriched {
                theme::winrate_color(p.winrate)
            } else {
                theme::TEXT_MUTED
            };
            text_cell(ui, &t, WR_W, &f, clr);
        }

        if c.earned_rr {
            if p.enriched {
                delta_rr_cell(ui, p, ERR_W, &f);
            } else {
                text_cell(ui, &loading, ERR_W, &f, theme::TEXT_MUTED);
            }
        }

        if c.level {
            let t = if p.account_level > 0 {
                p.account_level.to_string()
            } else {
                "-".into()
            };
            let clr = if p.account_level > 0 {
                theme::level_color(p.account_level)
            } else {
                theme::TEXT_MUTED
            };
            text_cell(ui, &t, LVL_W, &f, clr);
        }

        if show_skin {
            let skin_name = format_skin_name(
                &p.skin_name,
                &config.overlay.weapon,
                config.overlay.truncate_skins,
            );
            text_cell(ui, &skin_name, SKIN_W, &f, theme::TEXT_SECONDARY);
        }
    });
}

fn text_cell(ui: &mut Ui, text: &str, w: f32, font: &egui::FontId, color: egui::Color32) {
    let mut job = LayoutJob::default();
    job.append(
        text,
        0.0,
        TextFormat {
            font_id: font.clone(),
            color,
            ..Default::default()
        },
    );
    layout_job_cell(ui, job, w, color);
}

fn layout_job_cell(ui: &mut Ui, job: LayoutJob, w: f32, fallback_color: egui::Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, ROW_H), egui::Sense::hover());
    let clip_rect = Rect::from_min_max(
        Pos2::new(rect.left() + CELL_PAD, rect.top()),
        Pos2::new(rect.right() - CELL_PAD, rect.bottom()),
    );
    let galley = ui.painter().layout_job(job);
    if galley.size().x <= 0.0 {
        return;
    }

    let painter = ui.painter().with_clip_rect(clip_rect);
    painter.galley(
        Pos2::new(clip_rect.left(), rect.center().y - galley.size().y / 2.0),
        galley,
        fallback_color,
    );
}

fn delta_rr_cell(ui: &mut Ui, player: &PlayerDisplayData, w: f32, font: &egui::FontId) {
    let mut job = LayoutJob::default();
    let base = TextFormat {
        font_id: font.clone(),
        color: theme::TEXT_MUTED,
        ..Default::default()
    };

    if !player.has_comp_update || (player.earned_rr == 0 && player.afk_penalty == 0) {
        job.append("-", 0.0, base);
        layout_job_cell(ui, job, w, theme::TEXT_MUTED);
        return;
    }

    let rr_prefix = if player.earned_rr > 0 { "+" } else { "" };
    let rr_text = format!("{rr_prefix}{}", player.earned_rr);
    job.append(
        &rr_text,
        0.0,
        TextFormat {
            font_id: font.clone(),
            color: theme::rr_change_color(player.earned_rr),
            ..Default::default()
        },
    );
    job.append(
        " ",
        0.0,
        TextFormat {
            font_id: font.clone(),
            color: theme::TEXT_SECONDARY,
            ..Default::default()
        },
    );
    job.append(
        &format!("({})", player.afk_penalty),
        0.0,
        TextFormat {
            font_id: font.clone(),
            color: theme::rr_penalty_color(player.afk_penalty),
            ..Default::default()
        },
    );
    layout_job_cell(ui, job, w, theme::TEXT_PRIMARY);
}

fn state_color(state: &GameState) -> egui::Color32 {
    match state {
        GameState::WaitingForClient => theme::STATUS_WAITING,
        GameState::Menu => theme::STATUS_MENU,
        GameState::Pregame { .. } => theme::STATUS_PREGAME,
        GameState::Ingame { .. } => theme::STATUS_INGAME,
    }
}

fn loading_dots(ctx: &egui::Context) -> String {
    // Keep it ASCII so it renders consistently on the overlay.
    let t = ctx.input(|i| i.time);
    let phase = ((t * 3.0) as i32).rem_euclid(3);
    match phase {
        0 => ".".to_string(),
        1 => "..".to_string(),
        _ => "...".to_string(),
    }
}

fn selected_weapon_label(weapon_name: &str) -> String {
    weapon_name.trim().to_ascii_uppercase()
}

fn format_skin_name(raw_skin_name: &str, weapon_name: &str, truncate_skins: bool) -> String {
    let raw_skin_name = raw_skin_name.trim();
    if raw_skin_name.is_empty() {
        return String::new();
    }
    if !truncate_skins {
        return raw_skin_name.to_string();
    }

    let shortened = strip_weapon_suffix(raw_skin_name, weapon_name).trim();
    if shortened.is_empty() || shortened.eq_ignore_ascii_case("standard") {
        return raw_skin_name.to_string();
    }

    let mut tokens = shortened.split_whitespace();
    let Some(first) = tokens.next() else {
        return raw_skin_name.to_string();
    };

    let numeric_suffix = shortened.split_whitespace().rev().find(|token| {
        token.chars().any(|ch| ch.is_ascii_digit()) && !token.eq_ignore_ascii_case(first)
    });

    match numeric_suffix {
        Some(last) => format!("{first} {last}"),
        None => first.to_string(),
    }
}

fn strip_weapon_suffix<'a>(skin_name: &'a str, weapon_name: &str) -> &'a str {
    let suffix = format!(" {}", weapon_name.trim());
    if suffix.len() <= 1 {
        return skin_name;
    }

    if skin_name
        .to_ascii_lowercase()
        .ends_with(&suffix.to_ascii_lowercase())
    {
        let trimmed_len = skin_name.len().saturating_sub(suffix.len());
        skin_name[..trimmed_len].trim_end()
    } else {
        skin_name
    }
}

#[cfg(test)]
mod tests {
    use super::{format_skin_name, skin_column_visible, split_players_by_team};
    use crate::config::ColumnConfig;
    use crate::game::state::GameState;
    use crate::riot::types::PlayerDisplayData;

    fn player(puuid: &str, team_id: &str) -> PlayerDisplayData {
        PlayerDisplayData {
            puuid: puuid.into(),
            team_id: team_id.into(),
            ..Default::default()
        }
    }

    #[test]
    fn uses_local_player_team_when_first_player_is_enemy() {
        let players = vec![
            player("enemy-1", "Red"),
            player("self", "Blue"),
            player("ally-1", "Blue"),
            player("enemy-2", "Red"),
        ];

        let (allies, enemies) = split_players_by_team(&players, "self");

        assert_eq!(allies.len(), 2);
        assert!(allies.iter().all(|player| player.team_id == "Blue"));
        assert_eq!(enemies.len(), 2);
        assert!(enemies.iter().all(|player| player.team_id == "Red"));
    }

    #[test]
    fn truncates_skin_names_like_vry() {
        assert_eq!(
            format_skin_name("Prelude to Chaos Vandal", "Vandal", true),
            "Prelude"
        );
        assert_eq!(
            format_skin_name("RGX 11z Pro Vandal", "Vandal", true),
            "RGX 11z"
        );
        assert_eq!(format_skin_name("Standard", "Vandal", true), "Standard");
    }

    #[test]
    fn keeps_full_skin_name_when_disabled() {
        assert_eq!(
            format_skin_name("Prelude to Chaos Vandal", "Vandal", false),
            "Prelude to Chaos Vandal"
        );
    }

    #[test]
    fn only_shows_skin_column_ingame() {
        let columns = ColumnConfig {
            skin: true,
            ..Default::default()
        };

        assert!(!skin_column_visible(
            &columns,
            &GameState::Pregame {
                match_id: "pregame".into(),
            }
        ));
        assert!(skin_column_visible(
            &columns,
            &GameState::Ingame {
                match_id: "coregame".into(),
            }
        ));
    }
}
