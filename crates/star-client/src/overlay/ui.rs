use crate::config::{ColumnConfig, Config};
use crate::game::match_data::MatchContext;
use crate::game::state::GameState;
use crate::overlay::theme;
use crate::riot::types::{rank_color, PlayerDisplayData};
use chrono::{DateTime, NaiveDateTime, Utc};
use egui::text::{LayoutJob, TextFormat};
use egui::{Align, Align2, Layout, Pos2, Rect, RichText, Stroke, Ui, Vec2};

const PARTY_W: f32 = 8.0;
const STAR_W: f32 = 18.0;
const AGENT_W: f32 = 62.0;
const NAME_W: f32 = 125.0;
const RANK_W: f32 = 82.0;
const RANK_W_TRUNCATED: f32 = 64.0;
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
const SKIN_UPGRADE_GAP: f32 = 8.0;
const SKIN_UPGRADE_BAR_HEIGHT: f32 = 9.0;
const SKIN_UPGRADE_BAR_WIDTH: f32 = 1.5;
const SKIN_UPGRADE_DOT_RADIUS: f32 = 1.7;

pub fn render_overlay(
    ctx: &egui::Context,
    game_state: &GameState,
    players: &[PlayerDisplayData],
    match_context: Option<&MatchContext>,
    local_puuid: &str,
    config: &Config,
) {
    if players.is_empty() {
        return;
    }

    let columns = &config.columns;
    let screen = ctx.screen_rect();
    let show_leaderboard = leaderboard_column_visible(config, players);
    let show_skin = skin_column_visible(columns, game_state);
    let tw = table_width(config, show_leaderboard, show_skin);
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
                    title_bar(ui, game_state, match_context, config);
                    ui.add_space(4.0);
                    header_row(
                        ui,
                        columns,
                        config,
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

                    if config.features.last_played {
                        last_played_section(ui, players, local_puuid);
                    }
                });
        });
}

fn split_players_by_team<'a>(
    players: &'a [PlayerDisplayData],
    local_puuid: &str,
) -> (Vec<&'a PlayerDisplayData>, Vec<&'a PlayerDisplayData>) {
    let my_team = local_team_id(players, local_puuid);

    players
        .iter()
        .partition(|player| player.team_id == my_team || my_team.is_empty())
}

fn leaderboard_column_visible(config: &Config, players: &[PlayerDisplayData]) -> bool {
    if !config.columns.leaderboard {
        return false;
    }

    players.iter().any(|p| p.leaderboard_position > 0)
}

fn skin_column_visible(c: &ColumnConfig, state: &GameState) -> bool {
    c.skin && matches!(state, GameState::Ingame { .. })
}

fn table_width(config: &Config, show_leaderboard: bool, show_skin: bool) -> f32 {
    let c = &config.columns;
    let mut w = PARTY_W + STAR_W + AGENT_W + NAME_W + rank_column_width(config);
    if rr_column_visible(config) {
        w += RR_W;
    }
    if c.peak_rank {
        w += peak_column_width(config);
    }
    if c.previous_rank {
        w += previous_rank_column_width(config);
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

fn title_bar(
    ui: &mut Ui,
    state: &GameState,
    match_context: Option<&MatchContext>,
    config: &Config,
) {
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
            if config.features.server_id {
                if let Some(server_id) = match_context
                    .map(|context| context.server_id.as_str())
                    .filter(|server_id| !server_id.is_empty())
                {
                    ui.label(
                        RichText::new(format_server_id(server_id))
                            .font(theme::small_font())
                            .color(theme::TEXT_MUTED),
                    );
                }
            }
        });
    });
}

fn header_row(
    ui: &mut Ui,
    c: &ColumnConfig,
    config: &Config,
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
        hdr_cell(ui, "RANK", rank_column_width(config), &f, clr);
        if rr_column_visible(config) {
            hdr_cell(ui, "RR", RR_W, &f, clr);
        }
        if c.peak_rank {
            hdr_cell(ui, "PEAK", peak_column_width(config), &f, clr);
        }
        if c.previous_rank {
            hdr_cell(ui, "PREV", previous_rank_column_width(config), &f, clr);
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
    let rank_w = rank_column_width(config);
    let peak_w = peak_column_width(config);
    let prev_w = previous_rank_column_width(config);
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
        let display_name = if config.features.truncate_names {
            ellipsize(&name, 18)
        } else {
            name
        };
        text_cell(
            ui,
            &display_name,
            NAME_W,
            &f,
            theme::team_text_color(is_ally),
        );

        // Rank (always shown)
        if p.enriched {
            rank_cell(ui, p.current_rank, config, rank_w, &f, rank_color(p.current_rank));
        } else {
            text_cell(ui, &loading, rank_w, &f, theme::TEXT_MUTED);
        }

        if rr_column_visible(config) {
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
                if p.peak_rank > 0 {
                    rank_cell(ui, p.peak_rank, config, peak_w, &f, rank_color(p.peak_rank));
                } else {
                    text_cell(ui, "-", peak_w, &f, rank_color(p.peak_rank));
                }
            } else {
                text_cell(ui, &loading, peak_w, &f, theme::TEXT_MUTED);
            }
        }

        if c.previous_rank {
            if p.enriched {
                if p.previous_rank > 0 {
                    rank_cell(
                        ui,
                        p.previous_rank,
                        config,
                        prev_w,
                        &f,
                        rank_color(p.previous_rank),
                    );
                } else {
                    text_cell(ui, "-", prev_w, &f, rank_color(p.previous_rank));
                }
            } else {
                text_cell(ui, &loading, prev_w, &f, theme::TEXT_MUTED);
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
            skin_cell(
                ui,
                &skin_name,
                p.skin_level,
                p.skin_level_total,
                p.skin_color,
                SKIN_W,
                &f,
            );
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

fn rank_cell(
    ui: &mut Ui,
    tier: i32,
    config: &Config,
    w: f32,
    font: &egui::FontId,
    color: egui::Color32,
) {
    let (label, suffix) = format_rank_parts(tier, config);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, ROW_H), egui::Sense::hover());
    let clip_rect = Rect::from_min_max(
        Pos2::new(rect.left() + CELL_PAD, rect.top()),
        Pos2::new(rect.right() - CELL_PAD, rect.bottom()),
    );
    let painter = ui.painter();

    let mut label_job = LayoutJob::default();
    label_job.append(
        &label,
        0.0,
        TextFormat {
            font_id: font.clone(),
            color,
            ..Default::default()
        },
    );
    let label_galley = painter.layout_job(label_job);
    if label_galley.size().x <= 0.0 {
        return;
    }

    let Some(suffix) = suffix else {
        painter.with_clip_rect(clip_rect).galley(
            Pos2::new(clip_rect.left(), rect.center().y - label_galley.size().y / 2.0),
            label_galley,
            color,
        );
        return;
    };

    let mut suffix_job = LayoutJob::default();
    suffix_job.append(
        &suffix,
        0.0,
        TextFormat {
            font_id: font.clone(),
            color,
            ..Default::default()
        },
    );
    let suffix_galley = painter.layout_job(suffix_job);
    let suffix_width = suffix_galley.size().x;
    let suffix_left = (clip_rect.right() - suffix_width).max(clip_rect.left());
    let left_limit = (suffix_left - 4.0).max(clip_rect.left());
    let label_clip_rect = Rect::from_min_max(
        clip_rect.min,
        Pos2::new(left_limit, clip_rect.bottom()),
    );

    if label_clip_rect.width() > 0.0 {
        painter.with_clip_rect(label_clip_rect).galley(
            Pos2::new(label_clip_rect.left(), rect.center().y - label_galley.size().y / 2.0),
            label_galley,
            color,
        );
    }

    painter.with_clip_rect(clip_rect).galley(
        Pos2::new(suffix_left, rect.center().y - suffix_galley.size().y / 2.0),
        suffix_galley,
        color,
    );
}

fn skin_cell(
    ui: &mut Ui,
    skin_name: &str,
    skin_level: usize,
    skin_level_total: usize,
    skin_color: egui::Color32,
    w: f32,
    font: &egui::FontId,
) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, ROW_H), egui::Sense::hover());
    let clip_rect = Rect::from_min_max(
        Pos2::new(rect.left() + CELL_PAD, rect.top()),
        Pos2::new(rect.right() - CELL_PAD, rect.bottom()),
    );
    let painter = ui.painter().with_clip_rect(clip_rect);
    let bar_width = skin_upgrade_bar_width(skin_level_total);
    let bar_rect = if bar_width > 0.0 && bar_width < clip_rect.width() {
        Some(Rect::from_min_max(
            Pos2::new(clip_rect.right() - bar_width, clip_rect.top()),
            Pos2::new(clip_rect.right(), clip_rect.bottom()),
        ))
    } else {
        None
    };
    let name_clip_rect = Rect::from_min_max(
        clip_rect.min,
        Pos2::new(
            bar_rect
                .map(|rect| (rect.left() - 8.0).max(clip_rect.left()))
                .unwrap_or(clip_rect.right()),
            clip_rect.bottom(),
        ),
    );

    let mut name_job = LayoutJob::default();
    name_job.append(
        skin_name,
        0.0,
        TextFormat {
            font_id: font.clone(),
            color: skin_color,
            ..Default::default()
        },
    );
    let name_galley = ui.painter().layout_job(name_job);
    if name_galley.size().x > 0.0 {
        ui.painter().with_clip_rect(name_clip_rect).galley(
            Pos2::new(
                name_clip_rect.left(),
                rect.center().y - name_galley.size().y / 2.0,
            ),
            name_galley.clone(),
            skin_color,
        );
    }

    let Some(bar_rect) = bar_rect else {
        return;
    };

    paint_skin_upgrade_bar(&painter, bar_rect, skin_level, skin_level_total, skin_color);
}

fn paint_skin_upgrade_bar(
    painter: &egui::Painter,
    rect: Rect,
    skin_level: usize,
    skin_level_total: usize,
    skin_color: egui::Color32,
) {
    let clamped_level = skin_level.min(skin_level_total);
    let center_y = rect.center().y;
    let inactive_color = skin_color.gamma_multiply(0.4);

    for index in 0..skin_level_total {
        let x = rect.left() + index as f32 * SKIN_UPGRADE_GAP;
        if x > rect.right() {
            break;
        }

        if index < clamped_level {
            painter.line_segment(
                [
                    Pos2::new(x, center_y - SKIN_UPGRADE_BAR_HEIGHT / 2.0),
                    Pos2::new(x, center_y + SKIN_UPGRADE_BAR_HEIGHT / 2.0),
                ],
                Stroke::new(SKIN_UPGRADE_BAR_WIDTH, skin_color),
            );
        } else {
            painter.circle_filled(
                Pos2::new(x, center_y),
                SKIN_UPGRADE_DOT_RADIUS,
                inactive_color,
            );
        }
    }
}

fn skin_upgrade_bar_width(skin_level_total: usize) -> f32 {
    if skin_level_total <= 1 {
        0.0
    } else {
        (skin_level_total.saturating_sub(1) as f32 * SKIN_UPGRADE_GAP)
            + (SKIN_UPGRADE_DOT_RADIUS * 2.0).max(SKIN_UPGRADE_BAR_WIDTH)
    }
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

fn rr_column_visible(config: &Config) -> bool {
    config.columns.rr
}

fn rank_column_width(config: &Config) -> f32 {
    let base_width = rank_label_column_width(config);
    if config.columns.rr {
        base_width + 34.0
    } else {
        base_width
    }
}

fn peak_column_width(config: &Config) -> f32 {
    let _ = PEAK_W;
    rank_label_column_width(config)
}

fn previous_rank_column_width(config: &Config) -> f32 {
    let _ = PREV_W;
    rank_label_column_width(config)
}

fn rank_label_column_width(config: &Config) -> f32 {
    if config.features.truncate_ranks {
        RANK_W_TRUNCATED
    } else {
        RANK_W
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

fn format_rank_display(player: &PlayerDisplayData, config: &Config) -> String {
    format_rank_name(player.current_rank, config)
}

fn format_rank_parts(tier: i32, config: &Config) -> (String, Option<String>) {
    if tier <= 0 {
        return ("Unranked".to_string(), None);
    }

    let base = if config.features.truncate_ranks {
        truncated_rank_name(tier)
    } else {
        full_rank_name(tier)
    }
    .to_string();

    if tier == 27 {
        return (base, None);
    }

    let tier_number = ((tier - 3) % 3) + 1;
    let tier_label = if config.features.roman_numerals {
        roman_tier(tier_number)
    } else {
        tier_number.to_string()
    };

    if config.features.roman_numerals {
        (base, Some(tier_label))
    } else {
        (format!("{base} {tier_label}"), None)
    }
}

fn format_rank_name(tier: i32, config: &Config) -> String {
    let (label, suffix) = format_rank_parts(tier, config);
    match suffix {
        Some(suffix) => format!("{label} {suffix}"),
        None => label,
    }
}

fn truncated_rank_name(tier: i32) -> &'static str {
    match tier {
        3..=5 => "Irn",
        6..=8 => "Brz",
        9..=11 => "Slv",
        12..=14 => "Gld",
        15..=17 => "Plt",
        18..=20 => "Dia",
        21..=23 => "Asc",
        24..=26 => "Imm",
        27 => "Rad",
        _ => "Unranked",
    }
}

fn full_rank_name(tier: i32) -> &'static str {
    match tier {
        3..=5 => "Iron",
        6..=8 => "Bronze",
        9..=11 => "Silver",
        12..=14 => "Gold",
        15..=17 => "Platinum",
        18..=20 => "Diamond",
        21..=23 => "Ascendant",
        24..=26 => "Immortal",
        27 => "Radiant",
        _ => "Unranked",
    }
}

fn roman_tier(tier: i32) -> String {
    match tier {
        1 => "I".to_string(),
        2 => "II".to_string(),
        3 => "III".to_string(),
        _ => tier.to_string(),
    }
}

fn ellipsize(value: &str, max_chars: usize) -> String {
    let char_count = value.chars().count();
    if char_count <= max_chars || max_chars <= 3 {
        return value.to_string();
    }

    let truncated: String = value.chars().take(max_chars - 3).collect();
    format!("{truncated}...")
}

fn format_server_id(server_id: &str) -> String {
    let mut parts = server_id.split('.');
    let _ = parts.next();
    let _ = parts.next();
    let suffix: Vec<_> = parts.collect();
    if suffix.is_empty() {
        server_id.to_string()
    } else {
        suffix.join(".")
    }
}

fn last_played_section(ui: &mut Ui, players: &[PlayerDisplayData], local_puuid: &str) {
    let mut seen_before: Vec<_> = players
        .iter()
        .filter(|player| player.puuid != local_puuid && player.times_seen_before > 0)
        .collect();
    if seen_before.is_empty() {
        return;
    }

    seen_before.sort_by(|left, right| right.last_seen_at.cmp(&left.last_seen_at));

    ui.add_space(8.0);
    ui.label(
        RichText::new("LAST SEEN")
            .font(theme::small_font())
            .color(theme::TEXT_MUTED),
    );

    let my_team = local_team_id(players, local_puuid);
    for player in seen_before {
        let line = format_last_seen_summary(player, my_team);
        ui.label(
            RichText::new(line)
                .font(theme::small_font())
                .color(theme::TEXT_SECONDARY),
        );
    }
}

fn local_team_id<'a>(players: &'a [PlayerDisplayData], local_puuid: &str) -> &'a str {
    players
        .iter()
        .find(|player| player.puuid == local_puuid)
        .or_else(|| players.first())
        .map(|player| player.team_id.as_str())
        .unwrap_or("")
}

fn format_last_seen_summary(player: &PlayerDisplayData, my_team: &str) -> String {
    let previous_name = display_full_name(&player.last_seen_game_name, &player.last_seen_tag_line);
    let current_name = if player.is_incognito {
        None
    } else {
        display_full_name(&player.game_name, &player.tag_line)
    };
    let relation = team_relation_label(player, my_team);
    let agent = if player.agent_name.is_empty() {
        "unknown agent".to_string()
    } else {
        player.agent_name.clone()
    };

    let identity = if player.is_incognito {
        match previous_name {
            Some(previous_name) => format!("Ran into them as {previous_name}"),
            None => "Hidden now".to_string(),
        }
    } else {
        match (previous_name, current_name) {
            (Some(previous_name), Some(current_name)) if previous_name != current_name => {
                format!("{previous_name} is now {current_name}")
            }
            (_, Some(current_name)) => current_name,
            (Some(previous_name), None) => previous_name,
            (None, None) => "Unknown player".to_string(),
        }
    };

    let current_state = if player.is_incognito {
        format!("Hidden now on {relation} as {agent}")
    } else {
        format!("On {relation} as {agent}")
    };

    let age = format_history_age(&player.last_seen_at)
        .map(|age| format!("Last seen {age} ago"))
        .unwrap_or_else(|| "Last seen previously".to_string());
    let times_seen = player.times_seen_before.saturating_add(1);
    let seen_count = if times_seen == 1 {
        "Seen 1 time".to_string()
    } else {
        format!("Seen {times_seen} times")
    };

    format!("{identity}. {current_state}. {age}. {seen_count}.")
}

fn format_history_age(last_seen_at: &str) -> Option<String> {
    let parsed = NaiveDateTime::parse_from_str(last_seen_at, "%Y-%m-%d %H:%M:%S").ok()?;
    let parsed = DateTime::<Utc>::from_naive_utc_and_offset(parsed, Utc);
    let age = Utc::now().signed_duration_since(parsed);
    let seconds = age.num_seconds().max(0);

    if seconds < 60 {
        Some(format!("{seconds}s"))
    } else if seconds < 3_600 {
        Some(format!("{}m", seconds / 60))
    } else if seconds < 86_400 {
        Some(format!("{}h", seconds / 3_600))
    } else {
        Some(format!("{}d", seconds / 86_400))
    }
}

fn display_full_name(game_name: &str, tag_line: &str) -> Option<String> {
    let game_name = game_name.trim();
    let tag_line = tag_line.trim();
    if game_name.is_empty() {
        None
    } else if tag_line.is_empty() {
        Some(game_name.to_string())
    } else {
        Some(format!("{game_name}#{tag_line}"))
    }
}

fn team_relation_label(player: &PlayerDisplayData, my_team: &str) -> &'static str {
    if my_team.is_empty() || player.team_id.is_empty() {
        "unknown team"
    } else if player.team_id == my_team {
        "your team"
    } else {
        "enemy team"
    }
}

fn format_skin_name(raw_skin_name: &str, weapon_name: &str, truncate_skins: bool) -> String {
    let raw_skin_name = raw_skin_name.trim();
    if raw_skin_name.is_empty() {
        return String::new();
    }

    let standard_full_name = standard_skin_name(weapon_name);
    if raw_skin_name.eq_ignore_ascii_case("standard")
        || raw_skin_name.eq_ignore_ascii_case(&standard_full_name)
        || is_standard_weapon_name(raw_skin_name, weapon_name)
    {
        return if truncate_skins {
            "Standard".to_string()
        } else {
            standard_full_name
        };
    }

    if !truncate_skins {
        return raw_skin_name.to_string();
    }

    let shortened = strip_weapon_suffix(raw_skin_name, weapon_name).trim();
    if shortened.is_empty() || shortened.eq_ignore_ascii_case("standard") {
        return "Standard".to_string();
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

fn standard_skin_name(weapon_name: &str) -> String {
    let weapon_name = weapon_name.trim();
    if weapon_name.is_empty() {
        "Standard".to_string()
    } else {
        format!("Standard {weapon_name}")
    }
}

fn is_standard_weapon_name(raw_skin_name: &str, weapon_name: &str) -> bool {
    let weapon_name = weapon_name.trim();
    !weapon_name.is_empty() && raw_skin_name.eq_ignore_ascii_case(weapon_name)
}

#[cfg(test)]
mod tests {
    use super::{
        format_last_seen_summary, format_rank_display, format_rank_name, format_rank_parts,
        format_server_id, format_skin_name, leaderboard_column_visible, peak_column_width,
        previous_rank_column_width, rank_column_width, rr_column_visible, skin_column_visible,
        split_players_by_team,
    };
    use crate::config::{ColumnConfig, Config};
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
        assert_eq!(
            format_skin_name("Standard Vandal", "Vandal", true),
            "Standard"
        );
        assert_eq!(format_skin_name("Vandal", "Vandal", true), "Standard");
    }

    #[test]
    fn keeps_full_skin_name_when_disabled() {
        assert_eq!(
            format_skin_name("Prelude to Chaos Vandal", "Vandal", false),
            "Prelude to Chaos Vandal"
        );
        assert_eq!(
            format_skin_name("Standard", "Vandal", false),
            "Standard Vandal"
        );
        assert_eq!(
            format_skin_name("Vandal", "Vandal", false),
            "Standard Vandal"
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

    #[test]
    fn formats_rank_display_from_feature_flags() {
        let mut config = Config::default();
        config.columns.rr = true;
        config.features.truncate_ranks = true;
        config.features.roman_numerals = true;

        let player = PlayerDisplayData {
            current_rank: 25,
            rr: 87,
            ..Default::default()
        };

        assert_eq!(format_rank_display(&player, &config), "Imm II");
        assert!(rr_column_visible(&config));
        assert_eq!(rank_column_width(&config), 98.0);
        assert_eq!(peak_column_width(&config), 64.0);
        assert_eq!(previous_rank_column_width(&config), 64.0);
        assert_eq!(
            format_rank_parts(player.current_rank, &config),
            ("Imm".to_string(), Some("II".to_string()))
        );

        config.features.truncate_ranks = false;
        config.features.roman_numerals = false;
        assert_eq!(format_rank_name(16, &config), "Platinum 2");
        assert_eq!(rank_column_width(&config), 116.0);
        assert_eq!(peak_column_width(&config), 82.0);
        assert_eq!(previous_rank_column_width(&config), 82.0);
        assert_eq!(
            format_rank_parts(16, &config),
            ("Platinum 2".to_string(), None)
        );
    }

    #[test]
    fn leaderboard_column_hides_when_no_players_have_a_rank() {
        let mut config = Config::default();
        config.columns.leaderboard = true;
        assert!(!leaderboard_column_visible(
            &config,
            &[PlayerDisplayData::default()]
        ));

        assert!(leaderboard_column_visible(
            &config,
            &[PlayerDisplayData {
                leaderboard_position: 25,
                ..Default::default()
            }]
        ));
    }

    #[test]
    fn shortens_server_id_like_vry() {
        assert_eq!(format_server_id("aresriot.aws.ap.ne1"), "ap.ne1");
        assert_eq!(format_server_id("na"), "na");
    }

    #[test]
    fn last_seen_hidden_player_keeps_current_name_private() {
        let player = PlayerDisplayData {
            game_name: "HiddenCurrent".into(),
            tag_line: "NOW".into(),
            team_id: "Red".into(),
            agent_name: "Jett".into(),
            is_incognito: true,
            times_seen_before: 2,
            last_seen_at: "2026-03-09 12:00:00".into(),
            last_seen_game_name: "Example".into(),
            last_seen_tag_line: "TAG".into(),
            ..Default::default()
        };

        let summary = format_last_seen_summary(&player, "Blue");

        assert!(summary.contains("Ran into them as Example#TAG"));
        assert!(summary.contains("Hidden now on enemy team as Jett"));
        assert!(summary.contains("Seen 3 times"));
        assert!(!summary.contains("HiddenCurrent#NOW"));
        assert!(!summary.contains("HiddenCurrent"));
    }

    #[test]
    fn last_seen_visible_player_shows_rename_and_context() {
        let player = PlayerDisplayData {
            game_name: "Current".into(),
            tag_line: "NOW".into(),
            team_id: "Blue".into(),
            agent_name: "Sova".into(),
            times_seen_before: 1,
            last_seen_at: "2026-03-09 12:00:00".into(),
            last_seen_game_name: "Example".into(),
            last_seen_tag_line: "TAG".into(),
            ..Default::default()
        };

        let summary = format_last_seen_summary(&player, "Blue");

        assert!(summary.contains("Example#TAG is now Current#NOW"));
        assert!(summary.contains("On your team as Sova"));
        assert!(summary.contains("Seen 2 times"));
    }
}
