use crate::app::AppState;
use crate::config::Config;
use crate::game::players::{normalize_overlay_weapon, OVERLAY_WEAPONS};
use crate::overlay::hotkeys::{hotkey_name_from_egui, normalize_hotkey_name};
use crate::overlay::theme;
#[cfg(target_os = "windows")]
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;

pub const INITIAL_WINDOW_SIZE: [f32; 2] = [520.0, 620.0];
pub const MIN_WINDOW_SIZE: [u32; 2] = [360, 420];

#[derive(Default)]
pub struct SettingsState {
    baseline: Option<Config>,
    draft: Config,
    recording_hotkey: bool,
}

impl SettingsState {
    pub fn begin(&mut self, config: Config) {
        let config = settings_config(config);
        self.baseline = Some(config.clone());
        self.draft = config;
        self.recording_hotkey = false;
    }

    pub fn clear(&mut self) {
        self.baseline = None;
        self.recording_hotkey = false;
    }
}

pub fn render(
    ctx: &egui::Context,
    app_state: &Arc<RwLock<AppState>>,
    quit_flag: &Arc<AtomicBool>,
    settings_state: &mut SettingsState,
    _open: &mut bool,
) {
    configure_visuals(ctx);

    if settings_state.baseline.is_none() {
        let config = {
            let state = app_state.blocking_read();
            state.config.clone()
        };
        settings_state.begin(config);
    }

    handle_hotkey_recording(ctx, settings_state);

    if let Some(baseline) = settings_state.baseline.clone() {
        settings_state.draft.star.enabled = true;
        let dirty = settings_state.draft != baseline;
        let mut reset_requested = false;
        let mut save_requested = false;

        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(theme::BG_COLOR)
                    .inner_margin(egui::Margin::symmetric(14.0, 12.0)),
            )
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = egui::vec2(10.0, 8.0);
                ui.spacing_mut().button_padding = egui::vec2(10.0, 6.0);

                ui.horizontal(|ui| {
                    ui.heading(egui::RichText::new("Settings").color(theme::TEXT_PRIMARY));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add_enabled(dirty, egui::Button::new("Save")).clicked() {
                            save_requested = true;
                        }
                        if ui.add_enabled(dirty, egui::Button::new("Reset")).clicked() {
                            reset_requested = true;
                        }
                    });
                });
                ui.add_space(4.0);

                egui::ScrollArea::vertical()
                    .id_salt("settings_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.add_space(4.0);
                        settings_section(ui, "Columns", |ui| {
                            setting_checkbox(ui, &mut settings_state.draft.columns.skin, "Skin");
                            setting_checkbox(ui, &mut settings_state.draft.columns.rr, "RR");
                            setting_checkbox(
                                ui,
                                &mut settings_state.draft.columns.earned_rr,
                                "Earned RR",
                            );
                            setting_checkbox(
                                ui,
                                &mut settings_state.draft.columns.peak_rank,
                                "Peak Rank",
                            );
                            setting_checkbox(
                                ui,
                                &mut settings_state.draft.columns.previous_rank,
                                "Previous Rank",
                            );
                            setting_checkbox(
                                ui,
                                &mut settings_state.draft.columns.leaderboard,
                                "Leaderboard",
                            );
                            setting_checkbox(
                                ui,
                                &mut settings_state.draft.columns.headshot_percent,
                                "Headshot %",
                            );
                            setting_checkbox(
                                ui,
                                &mut settings_state.draft.columns.winrate,
                                "Winrate",
                            );
                            setting_checkbox(ui, &mut settings_state.draft.columns.kd, "K/D");
                            setting_checkbox(ui, &mut settings_state.draft.columns.level, "Level");
                        });

                        settings_section(ui, "Behavior", |ui| {
                            setting_checkbox(
                                ui,
                                &mut settings_state.draft.behavior.auto_show_pregame,
                                "Auto Show Pregame",
                            );
                            setting_checkbox(
                                ui,
                                &mut settings_state.draft.behavior.auto_hide_ingame,
                                "Auto Hide Ingame",
                            );
                            setting_checkbox(
                                ui,
                                &mut settings_state.draft.behavior.party_finder,
                                "Party Finder",
                            );
                            setting_checkbox(
                                ui,
                                &mut settings_state.draft.behavior.discord_rpc,
                                "Discord RPC",
                            );
                            setting_checkbox(
                                ui,
                                &mut settings_state.draft.behavior.auto_check_updates,
                                "Auto Check Updates",
                            );

                            #[cfg(target_os = "windows")]
                            setting_checkbox(
                                ui,
                                &mut settings_state.draft.behavior.launch_without_terminal,
                                "Hide Terminal (restart required)",
                            );
                        });

                        settings_section(ui, "Features", |ui| {
                            setting_checkbox(
                                ui,
                                &mut settings_state.draft.features.last_played,
                                "Last Played",
                            );
                            setting_checkbox(
                                ui,
                                &mut settings_state.draft.features.server_id,
                                "Server ID",
                            );
                            setting_checkbox(
                                ui,
                                &mut settings_state.draft.features.truncate_ranks,
                                "Truncate Ranks",
                            );
                            setting_checkbox(
                                ui,
                                &mut settings_state.draft.features.roman_numerals,
                                "Roman Numerals",
                            );
                            setting_checkbox(
                                ui,
                                &mut settings_state.draft.features.truncate_names,
                                "Truncate Names",
                            );
                            setting_checkbox(
                                ui,
                                &mut settings_state.draft.overlay.truncate_skins,
                                "Truncate Skins",
                            );
                        });

                        settings_section(ui, "Overlay", |ui| {
                            hotkey_recorder(ui, settings_state);

                            egui::ComboBox::from_label("Skin Weapon")
                                .selected_text(normalize_overlay_weapon(
                                    &settings_state.draft.overlay.weapon,
                                ))
                                .show_ui(ui, |ui| {
                                    for weapon in OVERLAY_WEAPONS {
                                        ui.selectable_value(
                                            &mut settings_state.draft.overlay.weapon,
                                            (*weapon).to_string(),
                                            *weapon,
                                        );
                                    }
                                });
                        });
                    });
            });

        if reset_requested {
            settings_state.draft = baseline.clone();
            settings_state.recording_hotkey = false;
        }

        if save_requested {
            let original = baseline.clone();
            let mut updated = settings_state.draft.clone();
            normalize_config_selections(&mut updated);
            let terminal_launch_changed = original.behavior.launch_without_terminal
                != updated.behavior.launch_without_terminal;
            if apply_config_change(
                app_state,
                quit_flag,
                original,
                updated.clone(),
                terminal_launch_changed,
            ) {
                settings_state.baseline = Some(updated.clone());
                settings_state.draft = updated;
                settings_state.recording_hotkey = false;
            }
        }
    }
}

fn configure_visuals(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = theme::BG_COLOR;
    visuals.window_fill = theme::BG_COLOR;
    visuals.widgets.noninteractive.fg_stroke.color = theme::TEXT_PRIMARY;
    visuals.widgets.inactive.fg_stroke.color = theme::TEXT_PRIMARY;
    visuals.widgets.hovered.fg_stroke.color = theme::TEXT_PRIMARY;
    visuals.widgets.active.fg_stroke.color = theme::TEXT_PRIMARY;
    visuals.selection.bg_fill = theme::TEAM_BLUE;
    ctx.set_visuals(visuals);
}

fn settings_section(ui: &mut egui::Ui, title: &str, add_contents: impl FnOnce(&mut egui::Ui)) {
    egui::CollapsingHeader::new(
        egui::RichText::new(title)
            .color(theme::TEXT_PRIMARY)
            .strong(),
    )
    .default_open(true)
    .show(ui, |ui| {
        ui.add_space(2.0);
        add_contents(ui);
        ui.add_space(4.0);
    });
    ui.add_space(6.0);
}

fn setting_checkbox(ui: &mut egui::Ui, value: &mut bool, label: &str) {
    ui.checkbox(value, egui::RichText::new(label).color(theme::TEXT_PRIMARY));
}

fn hotkey_recorder(ui: &mut egui::Ui, settings_state: &mut SettingsState) {
    ui.horizontal(|ui| {
        let button_text = if settings_state.recording_hotkey {
            "Press hotkey..."
        } else {
            settings_state.draft.overlay.hotkey.as_str()
        };
        if ui
            .add_sized([112.0, 30.0], egui::Button::new(button_text))
            .clicked()
        {
            settings_state.recording_hotkey = true;
        }
        ui.label(
            egui::RichText::new("Hotkey")
                .color(theme::TEXT_PRIMARY)
                .strong(),
        );
    });
}

fn handle_hotkey_recording(ctx: &egui::Context, settings_state: &mut SettingsState) {
    if !settings_state.recording_hotkey {
        return;
    }

    let captured = ctx.input(|input| {
        input.events.iter().find_map(|event| {
            let egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } = event
            else {
                return None;
            };

            if *key == egui::Key::Escape {
                return Some(None);
            }

            hotkey_name_from_egui(*key, *modifiers).map(Some)
        })
    });

    if let Some(captured) = captured {
        if let Some(hotkey) = captured {
            settings_state.draft.overlay.hotkey = hotkey;
        }
        settings_state.recording_hotkey = false;
    }
}

fn settings_config(mut config: Config) -> Config {
    normalize_config_selections(&mut config);
    config.star.enabled = true;
    config
}

fn normalize_config_selections(config: &mut Config) {
    config.overlay.hotkey = normalize_hotkey_name(&config.overlay.hotkey);
    config.overlay.weapon = normalize_overlay_weapon(&config.overlay.weapon).to_string();
    config.star.enabled = true;
}

fn apply_config_change(
    app_state: &Arc<RwLock<AppState>>,
    quit_flag: &Arc<AtomicBool>,
    original: Config,
    updated: Config,
    terminal_launch_changed: bool,
) -> bool {
    if let Err(error) = updated.save() {
        tracing::warn!("Failed to save settings: {}", error);
        return false;
    }

    {
        let mut state = app_state.blocking_write();
        state.config = updated.clone();
    }

    if terminal_launch_changed {
        restart_for_terminal_setting(app_state, quit_flag, original, updated);
    }

    true
}

#[cfg(target_os = "windows")]
fn restart_for_terminal_setting(
    app_state: &Arc<RwLock<AppState>>,
    quit_flag: &Arc<AtomicBool>,
    original: Config,
    updated: Config,
) {
    if let Err(error) = relaunch_current_process(updated.behavior.launch_without_terminal) {
        tracing::warn!(
            "Failed to restart app for terminal setting change: {}",
            error
        );
        restore_config(app_state, original);
        return;
    }

    quit_flag.store(true, Ordering::Relaxed);
}

#[cfg(not(target_os = "windows"))]
fn restart_for_terminal_setting(
    _app_state: &Arc<RwLock<AppState>>,
    _quit_flag: &Arc<AtomicBool>,
    _original: Config,
    _updated: Config,
) {
}

#[cfg(target_os = "windows")]
fn restore_config(app_state: &Arc<RwLock<AppState>>, config: Config) {
    if let Err(error) = config.save() {
        tracing::warn!(
            "Failed to restore settings after restart failure: {}",
            error
        );
    }

    let mut state = app_state.blocking_write();
    state.config = config;
}

#[cfg(target_os = "windows")]
fn relaunch_current_process(launch_without_terminal: bool) -> anyhow::Result<()> {
    use std::os::windows::process::CommandExt;
    use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;

    let exe = std::env::current_exe()?;
    let mut command = std::process::Command::new(exe);
    command.args(std::env::args_os().skip(1));

    if launch_without_terminal {
        command
            .creation_flags(CREATE_NO_WINDOW)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
    }

    command.spawn()?;
    Ok(())
}
