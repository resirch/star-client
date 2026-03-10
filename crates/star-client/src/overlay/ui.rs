use crate::config::ColumnConfig;
use crate::game::state::GameState;
use crate::overlay::theme;
use crate::riot::types::{rank_color, PlayerDisplayData};
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
const ERR_W: f32 = 50.0;
const LVL_W: f32 = 38.0;
const SKIN_W: f32 = 130.0;
const ROW_H: f32 = 22.0;
const HDR_H: f32 = 20.0;
const CELL_PAD: f32 = 3.0;

pub fn render_overlay(
    ctx: &egui::Context,
    game_state: &GameState,
    players: &[PlayerDisplayData],
    columns: &ColumnConfig,
) {
    if players.is_empty() {
        return;
    }

    let screen = ctx.screen_rect();
    let tw = table_width(columns);
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
                    header_row(ui, columns);
                    ui.add_space(2.0);

                    let my_team =
                        players.first().map(|p| p.team_id.as_str()).unwrap_or("");
                    let (allies, enemies): (Vec<_>, Vec<_>) = players
                        .iter()
                        .partition(|p| p.team_id == my_team || my_team.is_empty());

                    if !allies.is_empty() {
                        team_label(ui, "YOUR TEAM");
                        for p in &allies {
                            player_row(ui, p, columns, true);
                        }
                    }

                    if !enemies.is_empty() {
                        ui.add_space(6.0);
                        team_label(ui, "ENEMY TEAM");
                        for p in &enemies {
                            player_row(ui, p, columns, false);
                        }
                    }
                });
        });
}

fn table_width(c: &ColumnConfig) -> f32 {
    let mut w = PARTY_W + STAR_W + AGENT_W + NAME_W + RANK_W;
    if c.rr { w += RR_W; }
    if c.peak_rank { w += PEAK_W; }
    if c.previous_rank { w += PREV_W; }
    if c.leaderboard { w += LB_W; }
    if c.kd { w += KD_W; }
    if c.headshot_percent { w += HS_W; }
    if c.winrate { w += WR_W; }
    if c.earned_rr { w += ERR_W; }
    if c.level { w += LVL_W; }
    if c.skin { w += SKIN_W; }
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
                    .color(theme::TEXT_SECONDARY),
            );
        });
    });
}

fn header_row(ui: &mut Ui, c: &ColumnConfig) {
    let origin = ui.cursor().min;
    let full_w = ui.available_width();
    ui.painter()
        .rect_filled(Rect::from_min_size(origin, Vec2::new(full_w, HDR_H)), 2.0, theme::HEADER_BG);

    ui.horizontal(|ui| {
        ui.set_height(HDR_H);
        let f = theme::small_font();
        let clr = theme::TEXT_MUTED;

        hdr_cell(ui, "", PARTY_W, &f, clr);
        hdr_cell(ui, "", STAR_W, &f, clr);
        hdr_cell(ui, "AGENT", AGENT_W, &f, clr);
        hdr_cell(ui, "NAME", NAME_W, &f, clr);
        hdr_cell(ui, "RANK", RANK_W, &f, clr);
        if c.rr { hdr_cell(ui, "RR", RR_W, &f, clr); }
        if c.peak_rank { hdr_cell(ui, "PEAK", PEAK_W, &f, clr); }
        if c.previous_rank { hdr_cell(ui, "PREV", PREV_W, &f, clr); }
        if c.leaderboard { hdr_cell(ui, "#", LB_W, &f, clr); }
        if c.kd { hdr_cell(ui, "K/D", KD_W, &f, clr); }
        if c.headshot_percent { hdr_cell(ui, "HS%", HS_W, &f, clr); }
        if c.winrate { hdr_cell(ui, "WR%", WR_W, &f, clr); }
        if c.earned_rr { hdr_cell(ui, "ΔRR", ERR_W, &f, clr); }
        if c.level { hdr_cell(ui, "LVL", LVL_W, &f, clr); }
        if c.skin { hdr_cell(ui, "SKIN", SKIN_W, &f, clr); }
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
    ui.label(
        RichText::new(text)
            .font(theme::small_font())
            .color(theme::TEXT_MUTED),
    );
    ui.add_space(1.0);
}

fn player_row(ui: &mut Ui, p: &PlayerDisplayData, c: &ColumnConfig, is_ally: bool) {
    let bg = if is_ally { theme::ROW_BG_ALLY } else { theme::ROW_BG_ENEMY };
    let origin = ui.cursor().min;
    let full_w = ui.available_width();
    ui.painter()
        .rect_filled(Rect::from_min_size(origin, Vec2::new(full_w, ROW_H)), 2.0, bg);

    ui.horizontal(|ui| {
        ui.set_height(ROW_H);
        let f = theme::body_font();
        let loading = loading_dots(ui.ctx());

        // Party bar
        let (rect, _) = ui.allocate_exact_size(Vec2::new(PARTY_W, ROW_H), egui::Sense::hover());
        if p.party_number > 0 {
            let bar = Rect::from_center_size(rect.center(), Vec2::new(4.0, ROW_H - 4.0));
            ui.painter().rect_filled(bar, 2.0, theme::party_color(p.party_number));
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
        text_cell(ui, &p.agent_name, AGENT_W, &f, theme::TEXT_PRIMARY);

        // Name
        let name = if p.is_incognito {
            "---".into()
        } else {
            format!("{}#{}", p.game_name, p.tag_line)
        };
        text_cell(ui, &name, NAME_W, &f, theme::TEXT_PRIMARY);

        // Rank (always shown)
        if p.enriched {
            text_cell(ui, &p.rank_name, RANK_W, &f, rank_color(p.current_rank));
        } else {
            text_cell(ui, &loading, RANK_W, &f, theme::TEXT_MUTED);
        }

        if c.rr {
            let t = if p.enriched {
                if p.current_rank > 0 { format!("{} RR", p.rr) } else { "-".into() }
            } else {
                loading.clone()
            };
            text_cell(ui, &t, RR_W, &f, theme::TEXT_SECONDARY);
        }

        if c.peak_rank {
            if p.enriched {
                let t = if p.peak_rank > 0 { &p.peak_rank_name } else { "-" };
                text_cell(ui, t, PEAK_W, &f, rank_color(p.peak_rank));
            } else {
                text_cell(ui, &loading, PEAK_W, &f, theme::TEXT_MUTED);
            }
        }

        if c.previous_rank {
            if p.enriched {
                let t = if p.previous_rank > 0 { &p.previous_rank_name } else { "-" };
                text_cell(ui, t, PREV_W, &f, rank_color(p.previous_rank));
            } else {
                text_cell(ui, &loading, PREV_W, &f, theme::TEXT_MUTED);
            }
        }

        if c.leaderboard {
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
                if p.kd > 0.0 { format!("{:.2}", p.kd) } else { "-".into() }
            } else {
                loading.clone()
            };
            let clr = if p.enriched { theme::kd_color(p.kd) } else { theme::TEXT_MUTED };
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
            let clr = if p.enriched { theme::hs_color(p.headshot_percent) } else { theme::TEXT_MUTED };
            text_cell(ui, &t, HS_W, &f, clr);
        }

        if c.winrate {
            let t = if p.enriched {
                if p.games > 0 { format!("{:.0}%", p.winrate) } else { "-".into() }
            } else {
                loading.clone()
            };
            let clr = if p.enriched { theme::winrate_color(p.winrate) } else { theme::TEXT_MUTED };
            text_cell(ui, &t, WR_W, &f, clr);
        }

        if c.earned_rr {
            let t = if p.enriched {
                if p.has_comp_update {
                    format!("{}{}", if p.earned_rr > 0 { "+" } else { "" }, p.earned_rr)
                } else {
                    "-".into()
                }
            } else {
                loading.clone()
            };
            let clr = if p.enriched { theme::rr_change_color(p.earned_rr) } else { theme::TEXT_MUTED };
            text_cell(ui, &t, ERR_W, &f, clr);
        }

        if c.level {
            let t = if p.account_level > 0 { p.account_level.to_string() } else { "-".into() };
            text_cell(ui, &t, LVL_W, &f, theme::TEXT_SECONDARY);
        }

        if c.skin {
            text_cell(ui, &p.skin_name, SKIN_W, &f, theme::TEXT_SECONDARY);
        }
    });
}

fn text_cell(ui: &mut Ui, text: &str, w: f32, font: &egui::FontId, color: egui::Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, ROW_H), egui::Sense::hover());
    ui.painter().text(
        Pos2::new(rect.left() + CELL_PAD, rect.center().y),
        Align2::LEFT_CENTER,
        text,
        font.clone(),
        color,
    );
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
