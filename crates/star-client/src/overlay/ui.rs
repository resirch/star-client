use crate::assets;
use crate::config::{ColumnConfig, Config};
use crate::game::match_data::MatchContext;
use crate::game::state::GameState;
use crate::overlay::theme;
use crate::riot::types::{rank_color, PlayerDisplayData};
use chrono::{DateTime, NaiveDateTime, Utc};
use egui::text::{LayoutJob, TextFormat};
use egui::{Align, Align2, Color32, Layout, Pos2, Rect, RichText, Stroke, Ui, Vec2};
use std::cell::RefCell;

const PARTY_W: f32 = 8.0;
const STAR_W: f32 = 18.0;
const ROW_H: f32 = 22.0;
const HDR_H: f32 = 20.0;
const CELL_PAD: f32 = 3.0;
const SKIN_UPGRADE_GAP: f32 = 8.0;
const SKIN_UPGRADE_BAR_HEIGHT: f32 = 9.0;
const SKIN_UPGRADE_BAR_WIDTH: f32 = 1.5;
const SKIN_UPGRADE_DOT_RADIUS: f32 = 1.7;
const FRAME_INNER_MARGIN: f32 = 6.0;
const RANK_CELL_GAP: f32 = 4.0;
const OVERLAY_STAR_TEXTURE_SIZE: u32 = 64;
const STAR_ICON_INSET: f32 = 0.75;
const PLAYER_STAR_SIZE: f32 = 14.0;

thread_local! {
    static STAR_ICON_TEXTURE: RefCell<Option<Option<egui::TextureHandle>>> = const { RefCell::new(None) };
}

#[derive(Clone, Copy, Debug)]
struct ColumnWidths {
    party: f32,
    star: f32,
    agent: f32,
    name: f32,
    rank: f32,
    rr: f32,
    previous_rank: f32,
    peak_rank: f32,
    leaderboard: f32,
    kd: f32,
    headshot_percent: f32,
    winrate: f32,
    earned_rr: f32,
    level: f32,
    skin: f32,
}

#[derive(Clone, Copy, Debug)]
struct OverlayLayout {
    widths: ColumnWidths,
    show_leaderboard: bool,
    show_skin: bool,
    frame_width: f32,
}

pub fn render_overlay(
    ctx: &egui::Context,
    game_state: &GameState,
    players: &[PlayerDisplayData],
    match_context: Option<&MatchContext>,
    local_puuid: &str,
    config: &Config,
) {
    let columns = &config.columns;
    let visible_players = visible_players(game_state, players);
    let layout = overlay_layout(ctx, game_state, visible_players, columns, config);
    egui::Area::new(egui::Id::new("star_overlay"))
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .order(egui::Order::Foreground)
        .interactable(false)
        .show(ctx, |ui| {
            egui::Frame::none()
                .fill(theme::BG_COLOR)
                .rounding(theme::table_rounding())
                .stroke(theme::table_stroke())
                .inner_margin(FRAME_INNER_MARGIN)
                .show(ui, |ui: &mut Ui| {
                    ui.set_width(layout.frame_width);
                    ui.set_min_width(layout.frame_width);
                    ui.set_max_width(layout.frame_width);
                    title_bar(ui, game_state, match_context, config);
                    if visible_players.is_empty() {
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new("No active match data")
                                .font(theme::body_font())
                                .color(theme::TEXT_MUTED),
                        );
                    } else {
                        ui.add_space(4.0);
                        header_row(
                            ui,
                            columns,
                            layout.widths,
                            layout.show_leaderboard,
                            layout.show_skin,
                            &selected_weapon_label(&config.overlay.weapon),
                        );
                        ui.add_space(2.0);

                        let (allies, enemies) = split_players_by_team(visible_players, local_puuid);

                        if !allies.is_empty() {
                            team_label(ui, "YOUR TEAM", allies[0].team_id.as_str());
                            for p in &allies {
                                player_row(
                                    ui,
                                    p,
                                    config,
                                    layout.widths,
                                    true,
                                    layout.show_leaderboard,
                                    layout.show_skin,
                                );
                            }
                        }

                        if !enemies.is_empty() {
                            ui.add_space(6.0);
                            team_label(ui, "ENEMY TEAM", enemies[0].team_id.as_str());
                            for p in &enemies {
                                player_row(
                                    ui,
                                    p,
                                    config,
                                    layout.widths,
                                    false,
                                    layout.show_leaderboard,
                                    layout.show_skin,
                                );
                            }
                        }

                        if config.features.last_played {
                            last_played_section(ui, visible_players, local_puuid);
                        }
                    }
                });
        });
}

fn visible_players<'a>(
    game_state: &GameState,
    players: &'a [PlayerDisplayData],
) -> &'a [PlayerDisplayData] {
    if game_state.is_in_match() || matches!(game_state, GameState::Menu) {
        players
    } else {
        &[]
    }
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

fn overlay_layout(
    ctx: &egui::Context,
    game_state: &GameState,
    players: &[PlayerDisplayData],
    columns: &ColumnConfig,
    config: &Config,
) -> OverlayLayout {
    let show_leaderboard = leaderboard_column_visible(config, players);
    let show_skin = skin_column_visible(columns, game_state);
    let widths = measure_column_widths(ctx, players, config, show_leaderboard, show_skin);
    let frame_width = table_width(
        columns,
        widths,
        config,
        show_leaderboard,
        show_skin,
        ctx.style().spacing.item_spacing.x,
    );

    OverlayLayout {
        widths,
        show_leaderboard,
        show_skin,
        frame_width,
    }
}

fn table_width(
    columns: &ColumnConfig,
    widths: ColumnWidths,
    config: &Config,
    show_leaderboard: bool,
    show_skin: bool,
    item_spacing_x: f32,
) -> f32 {
    let mut width = widths.party + widths.star + widths.agent + widths.name + widths.rank;
    let mut column_count: usize = 5;
    if rr_column_visible(config) {
        width += widths.rr;
        column_count += 1;
    }
    if columns.previous_rank {
        width += widths.previous_rank;
        column_count += 1;
    }
    if columns.peak_rank {
        width += widths.peak_rank;
        column_count += 1;
    }
    if show_leaderboard {
        width += widths.leaderboard;
        column_count += 1;
    }
    if columns.kd {
        width += widths.kd;
        column_count += 1;
    }
    if columns.headshot_percent {
        width += widths.headshot_percent;
        column_count += 1;
    }
    if columns.winrate {
        width += widths.winrate;
        column_count += 1;
    }
    if columns.earned_rr {
        width += widths.earned_rr;
        column_count += 1;
    }
    if columns.level {
        width += widths.level;
        column_count += 1;
    }
    if show_skin {
        width += widths.skin;
        column_count += 1;
    }

    width + item_spacing_x * (column_count.saturating_sub(1) as f32)
}

fn measure_column_widths(
    ctx: &egui::Context,
    players: &[PlayerDisplayData],
    config: &Config,
    show_leaderboard: bool,
    show_skin: bool,
) -> ColumnWidths {
    let header_font = theme::small_font();
    let body_font = theme::body_font();
    let loading = loading_dots(ctx);
    let weapon_label = selected_weapon_label(&config.overlay.weapon);

    ColumnWidths {
        party: PARTY_W,
        star: STAR_W,
        agent: text_column_width(
            ctx,
            "AGENT",
            &header_font,
            &body_font,
            players.iter().map(|player| player.agent_name.clone()),
        ),
        name: text_column_width(
            ctx,
            "NAME",
            &header_font,
            &body_font,
            players
                .iter()
                .map(|player| player_display_name(player, config)),
        ),
        rank: rank_column_width(ctx, players, config, &header_font, &body_font, &loading),
        rr: text_column_width(
            ctx,
            "RR",
            &header_font,
            &body_font,
            players
                .iter()
                .map(|player| rr_column_value(player, &loading)),
        ),
        previous_rank: previous_rank_column_width(
            ctx,
            players,
            config,
            &header_font,
            &body_font,
            &loading,
        ),
        peak_rank: peak_column_width(ctx, players, config, &header_font, &body_font, &loading),
        leaderboard: if show_leaderboard {
            text_column_width(
                ctx,
                "#",
                &header_font,
                &body_font,
                players
                    .iter()
                    .map(|player| leaderboard_column_value(player, &loading)),
            )
        } else {
            0.0
        },
        kd: text_column_width(
            ctx,
            "K/D",
            &header_font,
            &body_font,
            players
                .iter()
                .map(|player| kd_column_value(player, &loading)),
        ),
        headshot_percent: text_column_width(
            ctx,
            "HS%",
            &header_font,
            &body_font,
            players
                .iter()
                .map(|player| headshot_column_value(player, &loading)),
        ),
        winrate: text_column_width(
            ctx,
            "WR%",
            &header_font,
            &body_font,
            players
                .iter()
                .map(|player| winrate_column_value(player, &loading)),
        ),
        earned_rr: text_column_width(
            ctx,
            "ΔRR",
            &header_font,
            &body_font,
            players
                .iter()
                .map(|player| earned_rr_column_value(player, &loading)),
        ),
        level: text_column_width(
            ctx,
            "LVL",
            &header_font,
            &body_font,
            players.iter().map(level_column_value),
        ),
        skin: skin_column_width(
            ctx,
            players,
            config,
            &weapon_label,
            &header_font,
            &body_font,
            show_skin,
        ),
    }
}

fn text_column_width<I>(
    ctx: &egui::Context,
    header: &str,
    header_font: &egui::FontId,
    body_font: &egui::FontId,
    values: I,
) -> f32
where
    I: IntoIterator<Item = String>,
{
    let mut width = measure_text_width(ctx, header_font, header);
    for value in values {
        width = width.max(measure_text_width(ctx, body_font, &value));
    }

    width + CELL_PAD * 2.0
}

fn rank_column_width(
    ctx: &egui::Context,
    players: &[PlayerDisplayData],
    config: &Config,
    header_font: &egui::FontId,
    body_font: &egui::FontId,
    loading: &str,
) -> f32 {
    rank_column_width_for(
        ctx,
        "RANK",
        body_font,
        header_font,
        players.iter().map(|player| {
            if player.enriched {
                player.current_rank
            } else {
                -1
            }
        }),
        config,
        loading,
    )
}

fn previous_rank_column_width(
    ctx: &egui::Context,
    players: &[PlayerDisplayData],
    config: &Config,
    header_font: &egui::FontId,
    body_font: &egui::FontId,
    loading: &str,
) -> f32 {
    rank_column_width_for(
        ctx,
        "PREV",
        body_font,
        header_font,
        players.iter().map(|player| {
            if player.enriched {
                player.previous_rank
            } else {
                -1
            }
        }),
        config,
        loading,
    )
}

fn peak_column_width(
    ctx: &egui::Context,
    players: &[PlayerDisplayData],
    config: &Config,
    header_font: &egui::FontId,
    body_font: &egui::FontId,
    loading: &str,
) -> f32 {
    rank_column_width_for(
        ctx,
        "PEAK",
        body_font,
        header_font,
        players.iter().map(|player| {
            if player.enriched {
                player.peak_rank
            } else {
                -1
            }
        }),
        config,
        loading,
    )
}

fn rank_column_width_for<I>(
    ctx: &egui::Context,
    header: &str,
    body_font: &egui::FontId,
    header_font: &egui::FontId,
    tiers: I,
    config: &Config,
    loading: &str,
) -> f32
where
    I: IntoIterator<Item = i32>,
{
    let mut width = measure_text_width(ctx, header_font, header);
    for tier in tiers {
        let value_width = if tier >= 0 {
            measure_rank_width(ctx, tier, config, body_font)
        } else {
            measure_text_width(ctx, body_font, loading)
        };
        width = width.max(value_width);
    }

    width + CELL_PAD * 2.0
}

fn skin_column_width(
    ctx: &egui::Context,
    players: &[PlayerDisplayData],
    config: &Config,
    header: &str,
    header_font: &egui::FontId,
    body_font: &egui::FontId,
    show_skin: bool,
) -> f32 {
    if !show_skin {
        return 0.0;
    }

    let mut width = measure_text_width(ctx, header_font, header);
    for player in players {
        let skin_name = format_skin_name(
            &player.skin_name,
            &config.overlay.weapon,
            config.overlay.truncate_skins,
        );
        let mut value_width = measure_text_width(ctx, body_font, &skin_name);
        let bar_width = skin_upgrade_bar_width(player.skin_level_total);
        if bar_width > 0.0 {
            value_width += 8.0 + bar_width;
        }
        width = width.max(value_width);
    }

    width + CELL_PAD * 2.0
}

fn measure_text_width(ctx: &egui::Context, font: &egui::FontId, text: &str) -> f32 {
    ctx.fonts(|fonts| {
        fonts
            .layout_no_wrap(text.to_string(), font.clone(), Color32::WHITE)
            .size()
            .x
    })
}

fn measure_rank_width(ctx: &egui::Context, tier: i32, config: &Config, font: &egui::FontId) -> f32 {
    let (label, suffix) = format_rank_parts(tier, config);
    let mut width = measure_text_width(ctx, font, &label);
    if let Some(suffix) = suffix {
        width += RANK_CELL_GAP + measure_text_width(ctx, font, &suffix);
    }
    width
}

fn title_bar(
    ui: &mut Ui,
    state: &GameState,
    match_context: Option<&MatchContext>,
    config: &Config,
) {
    let row_height = theme::header_font().size.max(theme::small_font().size);
    ui.horizontal(|ui| {
        ui.set_width(ui.max_rect().width());
        render_title_star_label(ui);
        let remaining_width = ui.available_width();
        ui.allocate_ui_with_layout(
            Vec2::new(remaining_width, row_height),
            Layout::right_to_left(Align::Center),
            |ui| {
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
            },
        );
    });
}

fn header_row(
    ui: &mut Ui,
    c: &ColumnConfig,
    widths: ColumnWidths,
    show_leaderboard: bool,
    show_skin: bool,
    skin_label: &str,
) {
    let origin = ui.cursor().min;
    let full_w = ui.max_rect().width();
    ui.painter().rect_filled(
        Rect::from_min_size(origin, Vec2::new(full_w, HDR_H)),
        2.0,
        theme::HEADER_BG,
    );

    ui.horizontal(|ui| {
        ui.set_height(HDR_H);
        let f = theme::small_font();
        let clr = theme::TEXT_MUTED;

        hdr_cell(ui, "", widths.party, &f, clr);
        hdr_cell(ui, "", widths.star, &f, clr);
        hdr_cell(ui, "AGENT", widths.agent, &f, clr);
        hdr_cell(ui, "NAME", widths.name, &f, clr);
        hdr_cell(ui, "RANK", widths.rank, &f, clr);
        if c.rr {
            hdr_cell(ui, "RR", widths.rr, &f, clr);
        }
        if c.previous_rank {
            hdr_cell(ui, "PREV", widths.previous_rank, &f, clr);
        }
        if c.peak_rank {
            hdr_cell(ui, "PEAK", widths.peak_rank, &f, clr);
        }
        if show_leaderboard {
            hdr_cell(ui, "#", widths.leaderboard, &f, clr);
        }
        if c.kd {
            hdr_cell(ui, "K/D", widths.kd, &f, clr);
        }
        if c.headshot_percent {
            hdr_cell(ui, "HS%", widths.headshot_percent, &f, clr);
        }
        if c.winrate {
            hdr_cell(ui, "WR%", widths.winrate, &f, clr);
        }
        if c.earned_rr {
            hdr_cell(ui, "ΔRR", widths.earned_rr, &f, clr);
        }
        if c.level {
            hdr_cell(ui, "LVL", widths.level, &f, clr);
        }
        if show_skin {
            hdr_cell(ui, skin_label, widths.skin, &f, clr);
        }
    });
}

fn hdr_cell(ui: &mut Ui, text: &str, w: f32, font: &egui::FontId, color: egui::Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, HDR_H), egui::Sense::hover());
    if !text.is_empty() {
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            text,
            font.clone(),
            color,
        );
    }
}

fn team_label(ui: &mut Ui, text: &str, team_id: &str) {
    ui.add_space(2.0);
    ui.label(
        RichText::new(text)
            .font(theme::small_font())
            .color(team_color(team_id, text.eq_ignore_ascii_case("YOUR TEAM"))),
    );
    ui.add_space(1.0);
}

fn player_row(
    ui: &mut Ui,
    p: &PlayerDisplayData,
    config: &Config,
    widths: ColumnWidths,
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
    let full_w = ui.max_rect().width();
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
        let (rect, _) =
            ui.allocate_exact_size(Vec2::new(widths.party, ROW_H), egui::Sense::hover());
        if p.party_number > 0 {
            let bar = Rect::from_center_size(rect.center(), Vec2::new(4.0, ROW_H - 4.0));
            ui.painter()
                .rect_filled(bar, 2.0, theme::party_color(p.party_number));
        }

        // Star
        let (rect, _) = ui.allocate_exact_size(Vec2::new(widths.star, ROW_H), egui::Sense::hover());
        if p.is_star_user {
            paint_star_icon(ui, rect, PLAYER_STAR_SIZE);
        }

        // Agent
        text_cell(
            ui,
            &p.agent_name,
            widths.agent,
            &f,
            theme::agent_color(&p.agent_name),
        );

        // Name
        let display_name = player_display_name(p, config);
        text_cell(
            ui,
            &display_name,
            widths.name,
            &f,
            team_color(&p.team_id, is_ally),
        );

        // Rank (always shown)
        if p.enriched {
            rank_cell(
                ui,
                p.current_rank,
                config,
                widths.rank,
                &f,
                rank_color(p.current_rank),
            );
        } else {
            text_cell(ui, &loading, widths.rank, &f, theme::TEXT_MUTED);
        }

        if rr_column_visible(config) {
            let t = rr_column_value(p, &loading);
            centered_text_cell(ui, &t, widths.rr, &f, theme::TEXT_SECONDARY);
        }

        if c.previous_rank {
            if p.enriched {
                if p.previous_rank > 0 {
                    rank_cell(
                        ui,
                        p.previous_rank,
                        config,
                        widths.previous_rank,
                        &f,
                        rank_color(p.previous_rank),
                    );
                } else {
                    text_cell(
                        ui,
                        "-",
                        widths.previous_rank,
                        &f,
                        rank_color(p.previous_rank),
                    );
                }
            } else {
                text_cell(ui, &loading, widths.previous_rank, &f, theme::TEXT_MUTED);
            }
        }

        if c.peak_rank {
            if p.enriched {
                if p.peak_rank > 0 {
                    rank_cell(
                        ui,
                        p.peak_rank,
                        config,
                        widths.peak_rank,
                        &f,
                        rank_color(p.peak_rank),
                    );
                } else {
                    text_cell(ui, "-", widths.peak_rank, &f, rank_color(p.peak_rank));
                }
            } else {
                text_cell(ui, &loading, widths.peak_rank, &f, theme::TEXT_MUTED);
            }
        }

        if show_leaderboard {
            let t = leaderboard_column_value(p, &loading);
            text_cell(ui, &t, widths.leaderboard, &f, theme::TEXT_SECONDARY);
        }

        if c.kd {
            let t = kd_column_value(p, &loading);
            let clr = if p.enriched {
                theme::kd_color(p.kd)
            } else {
                theme::TEXT_MUTED
            };
            centered_text_cell(ui, &t, widths.kd, &f, clr);
        }

        if c.headshot_percent {
            let t = headshot_column_value(p, &loading);
            let clr = if p.enriched {
                theme::hs_color(p.headshot_percent)
            } else {
                theme::TEXT_MUTED
            };
            centered_text_cell(ui, &t, widths.headshot_percent, &f, clr);
        }

        if c.winrate {
            let t = winrate_column_value(p, &loading);
            let clr = if p.enriched {
                theme::winrate_color(p.winrate)
            } else {
                theme::TEXT_MUTED
            };
            centered_text_cell(ui, &t, widths.winrate, &f, clr);
        }

        if c.earned_rr {
            if p.enriched {
                delta_rr_cell(ui, p, widths.earned_rr, &f);
            } else {
                centered_text_cell(ui, &loading, widths.earned_rr, &f, theme::TEXT_MUTED);
            }
        }

        if c.level {
            let t = level_column_value(p);
            let clr = if p.account_level > 0 {
                theme::level_color(p.account_level)
            } else {
                theme::TEXT_MUTED
            };
            centered_text_cell(ui, &t, widths.level, &f, clr);
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
                widths.skin,
                &f,
            );
        }
    });
}

fn render_title_star_label(ui: &mut Ui) {
    ui.label(
        RichText::new("STAR CLIENT")
            .font(theme::header_font())
            .color(theme::STAR_COLOR),
    );
}

fn paint_star_icon(ui: &mut Ui, rect: Rect, size: f32) {
    let available_size = Vec2::splat(size).min(rect.shrink(STAR_ICON_INSET).size());
    if let Some(texture) = overlay_star_texture(ui.ctx()) {
        let texture_size = texture.size_vec2();
        let scale = (available_size.x / texture_size.x)
            .min(available_size.y / texture_size.y)
            .max(0.0);
        let image_size = texture_size * scale;
        let image_rect = Rect::from_center_size(rect.center(), image_size);
        ui.painter().image(
            texture.id(),
            image_rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );
    } else {
        let image_rect = Rect::from_center_size(rect.center(), available_size);
        ui.painter().text(
            image_rect.center(),
            Align2::CENTER_CENTER,
            "★",
            theme::star_font(),
            theme::STAR_COLOR,
        );
    }
}

fn overlay_star_texture(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    STAR_ICON_TEXTURE.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.is_none() {
            *slot = Some(load_overlay_star_texture(ctx));
        }
        slot.as_ref().cloned().flatten()
    })
}

fn load_overlay_star_texture(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    match assets::overlay_star_image(OVERLAY_STAR_TEXTURE_SIZE) {
        Ok(image) => {
            Some(ctx.load_texture("overlay-star-icon", image, egui::TextureOptions::LINEAR))
        }
        Err(error) => {
            tracing::warn!("Failed to load overlay star icon: {}", error);
            None
        }
    }
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

fn centered_text_cell(ui: &mut Ui, text: &str, w: f32, font: &egui::FontId, color: egui::Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, ROW_H), egui::Sense::hover());
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        text,
        font.clone(),
        color,
    );
}

fn centered_layout_job_cell(ui: &mut Ui, job: LayoutJob, w: f32, fallback_color: egui::Color32) {
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
        Pos2::new(
            clip_rect.center().x - galley.size().x / 2.0,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        fallback_color,
    );
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
            Pos2::new(
                clip_rect.left(),
                rect.center().y - label_galley.size().y / 2.0,
            ),
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
    let label_clip_rect =
        Rect::from_min_max(clip_rect.min, Pos2::new(left_limit, clip_rect.bottom()));

    if label_clip_rect.width() > 0.0 {
        painter.with_clip_rect(label_clip_rect).galley(
            Pos2::new(
                label_clip_rect.left(),
                rect.center().y - label_galley.size().y / 2.0,
            ),
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
        centered_layout_job_cell(ui, job, w, theme::TEXT_MUTED);
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

    centered_layout_job_cell(ui, job, w, theme::TEXT_PRIMARY);
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

fn player_display_name(player: &PlayerDisplayData, config: &Config) -> String {
    let name = if player.is_incognito {
        "---".to_string()
    } else {
        format!("{}#{}", player.game_name, player.tag_line)
    };

    if config.features.truncate_names {
        ellipsize(&name, 18)
    } else {
        name
    }
}

fn rr_column_value(player: &PlayerDisplayData, loading: &str) -> String {
    if !player.enriched {
        loading.to_string()
    } else if player.current_rank > 0 {
        player.rr.to_string()
    } else {
        "-".to_string()
    }
}

fn leaderboard_column_value(player: &PlayerDisplayData, loading: &str) -> String {
    if !player.enriched {
        loading.to_string()
    } else if player.leaderboard_position > 0 {
        format!("#{}", player.leaderboard_position)
    } else {
        "-".to_string()
    }
}

fn kd_column_value(player: &PlayerDisplayData, loading: &str) -> String {
    if !player.enriched {
        loading.to_string()
    } else if player.kd > 0.0 {
        format!("{:.2}", player.kd)
    } else {
        "-".to_string()
    }
}

fn headshot_column_value(player: &PlayerDisplayData, loading: &str) -> String {
    if !player.enriched {
        loading.to_string()
    } else if player.headshot_percent > 0.0 {
        format!("{:.0}%", player.headshot_percent)
    } else {
        "-".to_string()
    }
}

fn winrate_column_value(player: &PlayerDisplayData, loading: &str) -> String {
    if !player.enriched {
        loading.to_string()
    } else if player.games > 0 {
        format!("{:.0}%", player.winrate)
    } else {
        "-".to_string()
    }
}

fn earned_rr_column_value(player: &PlayerDisplayData, loading: &str) -> String {
    if !player.enriched {
        loading.to_string()
    } else if !player.has_comp_update || (player.earned_rr == 0 && player.afk_penalty == 0) {
        "-".to_string()
    } else {
        let rr_prefix = if player.earned_rr > 0 { "+" } else { "" };
        format!("{rr_prefix}{} ({})", player.earned_rr, player.afk_penalty)
    }
}

fn level_column_value(player: &PlayerDisplayData) -> String {
    if player.account_level > 0 {
        player.account_level.to_string()
    } else {
        "-".to_string()
    }
}

fn format_rank_parts(tier: i32, config: &Config) -> (String, Option<String>) {
    if tier <= 0 {
        return ("-".to_string(), None);
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
        3..=5 => "IRN",
        6..=8 => "BRZ",
        9..=11 => "SLV",
        12..=14 => "GLD",
        15..=17 => "PLT",
        18..=20 => "DIA",
        21..=23 => "ASC",
        24..=26 => "IMM",
        27 => "RAD",
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
    let (local_party_id, local_party_number) = local_party_marker(players, local_puuid);
    let mut seen_before: Vec<_> = players
        .iter()
        .filter(|player| {
            player.puuid != local_puuid
                && player.times_seen_before > 0
                && !shares_local_party(player, local_party_id, local_party_number)
        })
        .collect();
    if seen_before.is_empty() {
        return;
    }

    seen_before.sort_by(|left, right| right.last_seen_at.cmp(&left.last_seen_at));

    ui.add_space(8.0);
    ui.label(
        RichText::new("LAST SEEN")
            .font(theme::small_regular_font())
            .color(theme::TEXT_MUTED),
    );

    let my_team = local_team_id(players, local_puuid);
    for player in seen_before {
        let line = format_last_seen_summary(player, my_team);
        ui.label(
            RichText::new(line)
                .font(theme::small_regular_font())
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

fn local_party_marker<'a>(players: &'a [PlayerDisplayData], local_puuid: &str) -> (&'a str, i32) {
    players
        .iter()
        .find(|player| player.puuid == local_puuid)
        .map(|player| (player.party_id.as_str(), player.party_number))
        .unwrap_or(("", 0))
}

fn shares_local_party(
    player: &PlayerDisplayData,
    local_party_id: &str,
    local_party_number: i32,
) -> bool {
    if !local_party_id.is_empty() {
        player.party_id == local_party_id
    } else {
        local_party_number > 0 && player.party_number == local_party_number
    }
}

fn format_last_seen_summary(player: &PlayerDisplayData, my_team: &str) -> String {
    let previous_name = display_full_name(&player.last_seen_game_name, &player.last_seen_tag_line);
    let current_name = display_full_name(&player.game_name, &player.tag_line);
    let relation = team_relation_label(player, my_team);
    let agent = if player.agent_name.is_empty() {
        "unknown agent".to_string()
    } else {
        player.agent_name.clone()
    };

    let identity = if player.is_incognito {
        format!("{agent} on {relation}")
    } else {
        match (&current_name, &previous_name) {
            (Some(current_name), _) => current_name.clone(),
            (None, Some(previous_name)) => previous_name.clone(),
            (None, None) => "Unknown player".to_string(),
        }
    };

    let previous_identity = match (&previous_name, &current_name) {
        (Some(previous_name), Some(current_name)) if previous_name != current_name => {
            Some(previous_name.clone())
        }
        _ => None,
    };

    let age = format_history_age(&player.last_seen_at)
        .map(|age| format!(" {age}"))
        .unwrap_or_else(|| " previously".to_string());
    let times_seen = player.times_seen_before.saturating_add(1);
    let seen_count = if times_seen == 1 {
        "(1 time)".to_string()
    } else {
        format!("({times_seen} times)")
    };
    let kd_suffix = player
        .last_seen_kd
        .map(|kd| format!(" KD {:.2}", kd))
        .unwrap_or_default();

    if let Some(previous_identity) = previous_identity {
        format!(
            "Last seen {identity} (previously {previous_identity}){age}{kd_suffix} {seen_count}"
        )
    } else {
        format!("Last seen {identity}{age}{kd_suffix} {seen_count}")
    }
}

fn format_history_age(last_seen_at: &str) -> Option<String> {
    let parsed = NaiveDateTime::parse_from_str(last_seen_at, "%Y-%m-%d %H:%M:%S").ok()?;
    let parsed = DateTime::<Utc>::from_naive_utc_and_offset(parsed, Utc);
    let age = Utc::now().signed_duration_since(parsed);
    let seconds = age.num_seconds().max(0);

    if seconds < 60 {
        Some(format!("{seconds} sec ago"))
    } else if seconds < 3_600 {
        Some(format!("{} min ago", seconds / 60))
    } else if seconds < 86_400 {
        Some(format!("{} hr ago", seconds / 3_600))
    } else {
        Some(format!("{} days ago", seconds / 86_400))
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

fn team_color(team_id: &str, is_ally: bool) -> egui::Color32 {
    if team_id.is_empty() {
        theme::team_text_color(is_ally)
    } else {
        theme::team_id_color(team_id)
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
        earned_rr_column_value, format_last_seen_summary, format_rank_display, format_rank_name,
        format_rank_parts, format_server_id, format_skin_name, leaderboard_column_visible,
        local_party_marker, player_display_name, rr_column_visible, shares_local_party,
        skin_column_visible, split_players_by_team,
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

        assert_eq!(format_rank_display(&player, &config), "IMM II");
        assert!(rr_column_visible(&config));
        assert_eq!(
            format_rank_parts(player.current_rank, &config),
            ("IMM".to_string(), Some("II".to_string()))
        );

        config.features.truncate_ranks = false;
        config.features.roman_numerals = false;
        assert_eq!(format_rank_name(16, &config), "Platinum 2");
        assert_eq!(
            format_rank_parts(16, &config),
            ("Platinum 2".to_string(), None)
        );
    }

    #[test]
    fn formats_unranked_as_dash() {
        let config = Config::default();

        assert_eq!(format_rank_name(0, &config), "-");
        assert_eq!(format_rank_parts(0, &config), ("-".to_string(), None));
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
    fn display_name_respects_privacy_and_truncation() {
        let mut config = Config::default();
        config.features.truncate_names = true;

        let private_player = PlayerDisplayData {
            is_incognito: true,
            ..Default::default()
        };
        assert_eq!(player_display_name(&private_player, &config), "---");

        let long_name_player = PlayerDisplayData {
            game_name: "VeryLongPlayerName".into(),
            tag_line: "ABCDEFG".into(),
            ..Default::default()
        };
        assert_eq!(
            player_display_name(&long_name_player, &config),
            "VeryLongPlayerN..."
        );
    }

    #[test]
    fn earned_rr_display_handles_positive_negative_and_empty_states() {
        let loading = "...";
        let loading_player = PlayerDisplayData::default();
        assert_eq!(earned_rr_column_value(&loading_player, loading), loading);

        let neutral_player = PlayerDisplayData {
            enriched: true,
            has_comp_update: false,
            ..Default::default()
        };
        assert_eq!(earned_rr_column_value(&neutral_player, loading), "-");

        let positive_player = PlayerDisplayData {
            enriched: true,
            has_comp_update: true,
            earned_rr: 24,
            afk_penalty: -3,
            ..Default::default()
        };
        assert_eq!(
            earned_rr_column_value(&positive_player, loading),
            "+24 (-3)"
        );
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
            last_seen_kd: Some(1.37),
            ..Default::default()
        };

        let summary = format_last_seen_summary(&player, "Blue");

        assert!(summary.contains("Last seen Jett on enemy team (previously Example#TAG)"));
        assert!(summary.contains("KD 1.37"));
        assert!(summary.contains("(3 times)"));
        assert!(!summary.contains("HiddenCurrent#NOW"));
        assert!(!summary.contains("HiddenCurrent"));
    }

    #[test]
    fn last_seen_visible_player_shows_rename() {
        let player = PlayerDisplayData {
            game_name: "Current".into(),
            tag_line: "NOW".into(),
            team_id: "Blue".into(),
            agent_name: "Sova".into(),
            times_seen_before: 1,
            last_seen_at: "2026-03-09 12:00:00".into(),
            last_seen_game_name: "Example".into(),
            last_seen_tag_line: "TAG".into(),
            last_seen_kd: Some(0.84),
            ..Default::default()
        };

        let summary = format_last_seen_summary(&player, "Blue");

        assert!(summary.contains("Last seen Current#NOW (previously Example#TAG)"));
        assert!(summary.contains("KD 0.84"));
        assert!(summary.contains("(2 times)"));
        assert!(!summary.contains("your team"));
    }

    #[test]
    fn last_seen_hidden_player_without_rename_uses_current_team_context_only() {
        let player = PlayerDisplayData {
            game_name: "Example".into(),
            tag_line: "TAG".into(),
            team_id: "Blue".into(),
            agent_name: "Jett".into(),
            is_incognito: true,
            last_seen_at: "2026-03-09 12:00:00".into(),
            last_seen_game_name: "Example".into(),
            last_seen_tag_line: "TAG".into(),
            ..Default::default()
        };

        let summary = format_last_seen_summary(&player, "Blue");

        assert!(summary.contains("Last seen Jett on your team"));
        assert!(summary.contains("(1 time)"));
        assert!(!summary.contains("previously"));
        assert!(!summary.contains("Example#TAG"));
    }

    #[test]
    fn last_seen_without_kd_omits_kd_suffix() {
        let player = PlayerDisplayData {
            game_name: "Current".into(),
            tag_line: "NOW".into(),
            team_id: "Blue".into(),
            times_seen_before: 1,
            last_seen_at: "2026-03-09 12:00:00".into(),
            last_seen_game_name: "Example".into(),
            last_seen_tag_line: "TAG".into(),
            ..Default::default()
        };

        let summary = format_last_seen_summary(&player, "Blue");

        assert!(!summary.contains("KD "));
    }

    #[test]
    fn local_party_marker_prefers_exact_local_player() {
        let players = vec![
            PlayerDisplayData {
                puuid: "other".into(),
                party_id: "party_2".into(),
                party_number: 2,
                ..Default::default()
            },
            PlayerDisplayData {
                puuid: "local".into(),
                party_id: "party_1".into(),
                party_number: 1,
                ..Default::default()
            },
        ];

        let (party_id, party_number) = local_party_marker(&players, "local");

        assert_eq!(party_id, "party_1");
        assert_eq!(party_number, 1);
    }

    #[test]
    fn shares_local_party_uses_party_id_when_available() {
        let teammate = PlayerDisplayData {
            party_id: "party_1".into(),
            party_number: 2,
            ..Default::default()
        };
        let outsider = PlayerDisplayData {
            party_id: "party_2".into(),
            party_number: 1,
            ..Default::default()
        };

        assert!(shares_local_party(&teammate, "party_1", 1));
        assert!(!shares_local_party(&outsider, "party_1", 1));
    }

    #[test]
    fn shares_local_party_falls_back_to_party_number() {
        let teammate = PlayerDisplayData {
            party_number: 3,
            ..Default::default()
        };
        let outsider = PlayerDisplayData {
            party_number: 1,
            ..Default::default()
        };

        assert!(shares_local_party(&teammate, "", 3));
        assert!(!shares_local_party(&outsider, "", 3));
    }
}
