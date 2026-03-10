use crate::config::ColumnConfig;
use crate::game::state::GameState;
use crate::overlay::theme;
use crate::riot::types::{rank_color, PlayerDisplayData};
use egui::{Align, Color32, Layout, Pos2, Rect, RichText, Ui, Vec2};

pub struct OverlayUi {
    pub visible: bool,
}

impl OverlayUi {
    pub fn new() -> Self {
        Self { visible: false }
    }

    pub fn render(
        &self,
        ctx: &egui::Context,
        game_state: &GameState,
        players: &[PlayerDisplayData],
        columns: &ColumnConfig,
    ) {
        if !self.visible || players.is_empty() {
            return;
        }

        let screen = ctx.screen_rect();
        let table_width = calculate_table_width(columns);
        let x_offset = (screen.width() - table_width) / 2.0;
        let y_offset = 60.0;

        egui::Area::new(egui::Id::new("star_overlay"))
            .fixed_pos(Pos2::new(x_offset, y_offset))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::none()
                    .fill(theme::BG_COLOR)
                    .rounding(theme::table_rounding())
                    .stroke(theme::table_stroke())
                    .inner_margin(4.0)
                    .show(ui, |ui: &mut Ui| {
                        ui.set_min_width(table_width);
                        render_title_bar(ui, game_state);
                        ui.add_space(2.0);
                        render_header(ui, columns);
                        ui.add_space(1.0);

                        let my_team = find_player_team(players);
                        let (allies, enemies) = split_teams(players, &my_team);

                        if !allies.is_empty() {
                            render_team_label(ui, "YOUR TEAM");
                            for player in &allies {
                                render_player_row(ui, player, columns, true);
                            }
                        }

                        if !enemies.is_empty() {
                            ui.add_space(4.0);
                            render_team_label(ui, "ENEMY TEAM");
                            for player in &enemies {
                                render_player_row(ui, player, columns, false);
                            }
                        }
                    });
            });
    }
}

fn calculate_table_width(columns: &ColumnConfig) -> f32 {
    let mut w = 200.0; // name + agent base
    if columns.rr { w += 65.0; }
    if columns.peak_rank { w += 80.0; }
    if columns.previous_rank { w += 80.0; }
    if columns.leaderboard { w += 45.0; }
    if columns.kd { w += 50.0; }
    if columns.headshot_percent { w += 50.0; }
    if columns.winrate { w += 55.0; }
    if columns.earned_rr { w += 55.0; }
    if columns.level { w += 40.0; }
    if columns.skin { w += 120.0; }
    w
}

fn render_title_bar(ui: &mut Ui, state: &GameState) {
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

fn render_header(ui: &mut Ui, columns: &ColumnConfig) {
    let header_rect = ui.available_rect_before_wrap();
    let header_rect = Rect::from_min_size(header_rect.min, Vec2::new(ui.available_width(), 20.0));
    ui.painter()
        .rect_filled(header_rect, 0.0, theme::HEADER_BG);

    ui.horizontal(|ui| {
        ui.set_height(20.0);
        let font = theme::small_font();
        let color = theme::TEXT_MUTED;

        header_cell(ui, "", 16.0, &font, color); // party indicator
        header_cell(ui, "", 14.0, &font, color); // star
        header_cell(ui, "AGENT", 55.0, &font, color);
        header_cell(ui, "NAME", 115.0, &font, color);
        header_cell(ui, "RANK", 0.0, &font, color); // rank is always shown

        if columns.rr { header_cell(ui, "RR", 65.0, &font, color); }
        if columns.peak_rank { header_cell(ui, "PEAK", 80.0, &font, color); }
        if columns.previous_rank { header_cell(ui, "PREV", 80.0, &font, color); }
        if columns.leaderboard { header_cell(ui, "#", 45.0, &font, color); }
        if columns.kd { header_cell(ui, "K/D", 50.0, &font, color); }
        if columns.headshot_percent { header_cell(ui, "HS%", 50.0, &font, color); }
        if columns.winrate { header_cell(ui, "WR%", 55.0, &font, color); }
        if columns.earned_rr { header_cell(ui, "ΔRR", 55.0, &font, color); }
        if columns.level { header_cell(ui, "LVL", 40.0, &font, color); }
        if columns.skin { header_cell(ui, "SKIN", 120.0, &font, color); }
    });
}

fn header_cell(ui: &mut Ui, text: &str, width: f32, font: &egui::FontId, color: Color32) {
    if width > 0.0 {
        ui.allocate_ui(Vec2::new(width, 20.0), |ui| {
            ui.label(RichText::new(text).font(font.clone()).color(color));
        });
    } else {
        ui.label(RichText::new(text).font(font.clone()).color(color));
    }
}

fn render_team_label(ui: &mut Ui, text: &str) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(text)
                .font(theme::small_font())
                .color(theme::TEXT_MUTED),
        );
    });
}

fn render_player_row(
    ui: &mut Ui,
    player: &PlayerDisplayData,
    columns: &ColumnConfig,
    is_ally: bool,
) {
    let bg = if is_ally {
        theme::ROW_BG_ALLY
    } else {
        theme::ROW_BG_ENEMY
    };

    let row_rect = ui.available_rect_before_wrap();
    let row_rect = Rect::from_min_size(row_rect.min, Vec2::new(ui.available_width(), 22.0));
    ui.painter().rect_filled(row_rect, 2.0, bg);

    ui.horizontal(|ui| {
        ui.set_height(22.0);
        let font = theme::body_font();

        // Party color indicator
        let party_col = theme::party_color(player.party_number);
        if player.party_number > 0 {
            let (rect, _) = ui.allocate_exact_size(Vec2::new(4.0, 18.0), egui::Sense::hover());
            ui.painter().rect_filled(rect, 2.0, party_col);
            ui.add_space(4.0);
        } else {
            ui.add_space(16.0);
        }

        // Star indicator
        if player.is_star_user {
            ui.label(
                RichText::new("★")
                    .font(theme::star_font())
                    .color(theme::STAR_COLOR),
            );
        } else {
            ui.add_space(14.0);
        }

        // Agent
        ui.allocate_ui(Vec2::new(55.0, 22.0), |ui| {
            ui.label(
                RichText::new(&player.agent_name)
                    .font(font.clone())
                    .color(theme::TEXT_PRIMARY),
            );
        });

        // Name
        ui.allocate_ui(Vec2::new(115.0, 22.0), |ui| {
            let name = if player.is_incognito {
                "---".to_string()
            } else {
                format!("{}#{}", player.game_name, player.tag_line)
            };
            ui.label(
                RichText::new(name)
                    .font(font.clone())
                    .color(theme::TEXT_PRIMARY),
            );
        });

        // Rank (always shown)
        let rank_col = rank_color(player.current_rank);
        ui.label(
            RichText::new(&player.rank_name)
                .font(font.clone())
                .color(rank_col),
        );

        // RR
        if columns.rr {
            ui.allocate_ui(Vec2::new(65.0, 22.0), |ui| {
                let rr_text = if player.current_rank > 0 {
                    format!("{} RR", player.rr)
                } else {
                    "-".into()
                };
                ui.label(
                    RichText::new(rr_text)
                        .font(font.clone())
                        .color(theme::TEXT_SECONDARY),
                );
            });
        }

        // Peak rank
        if columns.peak_rank {
            ui.allocate_ui(Vec2::new(80.0, 22.0), |ui| {
                let col = rank_color(player.peak_rank);
                ui.label(
                    RichText::new(&player.peak_rank_name)
                        .font(font.clone())
                        .color(col),
                );
            });
        }

        // Previous rank
        if columns.previous_rank {
            ui.allocate_ui(Vec2::new(80.0, 22.0), |ui| {
                let col = rank_color(player.previous_rank);
                ui.label(
                    RichText::new(&player.previous_rank_name)
                        .font(font.clone())
                        .color(col),
                );
            });
        }

        // Leaderboard
        if columns.leaderboard {
            ui.allocate_ui(Vec2::new(45.0, 22.0), |ui| {
                let text = if player.leaderboard_position > 0 {
                    format!("#{}", player.leaderboard_position)
                } else {
                    "-".into()
                };
                ui.label(
                    RichText::new(text)
                        .font(font.clone())
                        .color(theme::TEXT_SECONDARY),
                );
            });
        }

        // K/D
        if columns.kd {
            ui.allocate_ui(Vec2::new(50.0, 22.0), |ui| {
                let col = theme::kd_color(player.kd);
                let text = if player.kd > 0.0 {
                    format!("{:.2}", player.kd)
                } else {
                    "-".into()
                };
                ui.label(RichText::new(text).font(font.clone()).color(col));
            });
        }

        // HS%
        if columns.headshot_percent {
            ui.allocate_ui(Vec2::new(50.0, 22.0), |ui| {
                let col = theme::hs_color(player.headshot_percent);
                let text = if player.headshot_percent > 0.0 {
                    format!("{:.0}%", player.headshot_percent)
                } else {
                    "-".into()
                };
                ui.label(RichText::new(text).font(font.clone()).color(col));
            });
        }

        // Win rate
        if columns.winrate {
            ui.allocate_ui(Vec2::new(55.0, 22.0), |ui| {
                let col = theme::winrate_color(player.winrate);
                let text = if player.games > 0 {
                    format!("{:.0}%", player.winrate)
                } else {
                    "-".into()
                };
                ui.label(RichText::new(text).font(font.clone()).color(col));
            });
        }

        // Earned RR
        if columns.earned_rr {
            ui.allocate_ui(Vec2::new(55.0, 22.0), |ui| {
                let col = theme::rr_change_color(player.earned_rr);
                let text = if player.earned_rr != 0 {
                    let prefix = if player.earned_rr > 0 { "+" } else { "" };
                    format!("{}{}", prefix, player.earned_rr)
                } else {
                    "-".into()
                };
                ui.label(RichText::new(text).font(font.clone()).color(col));
            });
        }

        // Level
        if columns.level {
            ui.allocate_ui(Vec2::new(40.0, 22.0), |ui| {
                ui.label(
                    RichText::new(player.account_level.to_string())
                        .font(font.clone())
                        .color(theme::TEXT_SECONDARY),
                );
            });
        }

        // Skin
        if columns.skin {
            ui.allocate_ui(Vec2::new(120.0, 22.0), |ui| {
                ui.label(
                    RichText::new(&player.skin_name)
                        .font(font.clone())
                        .color(theme::TEXT_SECONDARY),
                );
            });
        }
    });
}

fn find_player_team(players: &[PlayerDisplayData]) -> String {
    // First player is assumed to be on our team (VRY convention)
    players
        .first()
        .map(|p| p.team_id.clone())
        .unwrap_or_default()
}

fn split_teams<'a>(
    players: &'a [PlayerDisplayData],
    my_team: &str,
) -> (Vec<&'a PlayerDisplayData>, Vec<&'a PlayerDisplayData>) {
    let mut allies = Vec::new();
    let mut enemies = Vec::new();

    for player in players {
        if player.team_id == my_team || my_team.is_empty() {
            allies.push(player);
        } else {
            enemies.push(player);
        }
    }

    (allies, enemies)
}
