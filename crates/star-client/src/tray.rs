use crate::app::AppState;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::{TrayIcon, TrayIconBuilder};

pub struct SystemTray {
    _tray: TrayIcon,
    quit_flag: Arc<AtomicBool>,
    toggle_items: HashMap<&'static str, CheckMenuItem>,
    quit_id: tray_icon::menu::MenuId,
}

impl SystemTray {
    pub fn new(
        app_state: Arc<RwLock<AppState>>,
        quit_flag: Arc<AtomicBool>,
    ) -> anyhow::Result<Self> {
        let config = {
            let state = app_state.blocking_read();
            state.config.clone()
        };

        let menu = Menu::new();
        let mut toggle_items: HashMap<&'static str, CheckMenuItem> = HashMap::new();

        let columns_menu = Submenu::new("Columns", true);
        append_toggle(
            &columns_menu,
            &mut toggle_items,
            "columns.skin",
            "Skin",
            config.columns.skin,
        )?;
        append_toggle(
            &columns_menu,
            &mut toggle_items,
            "columns.rr",
            "RR",
            config.columns.rr,
        )?;
        append_toggle(
            &columns_menu,
            &mut toggle_items,
            "columns.earned_rr",
            "Earned RR",
            config.columns.earned_rr,
        )?;
        append_toggle(
            &columns_menu,
            &mut toggle_items,
            "columns.peak_rank",
            "Peak Rank",
            config.columns.peak_rank,
        )?;
        append_toggle(
            &columns_menu,
            &mut toggle_items,
            "columns.previous_rank",
            "Previous Rank",
            config.columns.previous_rank,
        )?;
        append_toggle(
            &columns_menu,
            &mut toggle_items,
            "columns.leaderboard",
            "Leaderboard",
            config.columns.leaderboard,
        )?;
        append_toggle(
            &columns_menu,
            &mut toggle_items,
            "columns.headshot_percent",
            "Headshot %",
            config.columns.headshot_percent,
        )?;
        append_toggle(
            &columns_menu,
            &mut toggle_items,
            "columns.winrate",
            "Winrate",
            config.columns.winrate,
        )?;
        append_toggle(
            &columns_menu,
            &mut toggle_items,
            "columns.kd",
            "K/D",
            config.columns.kd,
        )?;
        append_toggle(
            &columns_menu,
            &mut toggle_items,
            "columns.level",
            "Level",
            config.columns.level,
        )?;

        let behavior_menu = Submenu::new("Behavior", true);
        append_toggle(
            &behavior_menu,
            &mut toggle_items,
            "behavior.auto_show_pregame",
            "Auto Show Pregame",
            config.behavior.auto_show_pregame,
        )?;
        append_toggle(
            &behavior_menu,
            &mut toggle_items,
            "behavior.auto_hide_ingame",
            "Auto Hide Ingame",
            config.behavior.auto_hide_ingame,
        )?;
        append_toggle(
            &behavior_menu,
            &mut toggle_items,
            "behavior.party_finder",
            "Party Finder",
            config.behavior.party_finder,
        )?;
        append_toggle(
            &behavior_menu,
            &mut toggle_items,
            "behavior.discord_rpc",
            "Discord RPC",
            config.behavior.discord_rpc,
        )?;

        let features_menu = Submenu::new("Features", true);
        append_toggle(
            &features_menu,
            &mut toggle_items,
            "features.last_played",
            "Last Played",
            config.features.last_played,
        )?;
        append_toggle(
            &features_menu,
            &mut toggle_items,
            "features.auto_hide_leaderboard",
            "Auto Hide Leaderboard",
            config.features.auto_hide_leaderboard,
        )?;
        append_toggle(
            &features_menu,
            &mut toggle_items,
            "features.peak_rank_act",
            "Peak Rank Act",
            config.features.peak_rank_act,
        )?;
        append_toggle(
            &features_menu,
            &mut toggle_items,
            "features.aggregate_rank_rr",
            "Aggregate Rank RR",
            config.features.aggregate_rank_rr,
        )?;
        append_toggle(
            &features_menu,
            &mut toggle_items,
            "features.server_id",
            "Server ID",
            config.features.server_id,
        )?;
        append_toggle(
            &features_menu,
            &mut toggle_items,
            "features.short_ranks",
            "Short Ranks",
            config.features.short_ranks,
        )?;
        append_toggle(
            &features_menu,
            &mut toggle_items,
            "features.truncate_names",
            "Truncate Names",
            config.features.truncate_names,
        )?;
        append_toggle(
            &features_menu,
            &mut toggle_items,
            "features.truncate_ranks",
            "Truncate Ranks",
            config.features.truncate_ranks,
        )?;
        append_toggle(
            &features_menu,
            &mut toggle_items,
            "features.roman_numerals",
            "Roman Numerals",
            config.features.roman_numerals,
        )?;
        let display_menu = Submenu::new("Display", true);
        append_toggle(
            &display_menu,
            &mut toggle_items,
            "overlay.truncate_skins",
            "Truncate Skins",
            config.overlay.truncate_skins,
        )?;

        let star_menu = Submenu::new("Star", true);
        append_toggle(
            &star_menu,
            &mut toggle_items,
            "star.enabled",
            "Enabled",
            config.star.enabled,
        )?;

        let quit_item = MenuItem::with_id("quit", "Quit Star Client", true, None);
        let quit_id = quit_item.id().clone();
        menu.append(&columns_menu)?;
        menu.append(&behavior_menu)?;
        menu.append(&features_menu)?;
        menu.append(&display_menu)?;
        menu.append(&star_menu)?;
        menu.append(&PredefinedMenuItem::separator())?;
        menu.append(&quit_item)?;

        let icon = load_tray_icon();

        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("Star Client")
            .with_icon(icon)
            .build()?;

        Ok(Self {
            _tray: tray,
            quit_flag,
            toggle_items,
            quit_id,
        })
    }

    pub fn poll_events(&self, app_state: &Arc<RwLock<AppState>>) {
        while let Ok(event) = MenuEvent::receiver().try_recv() {
            if event.id() == &self.quit_id {
                self.quit_flag.store(true, Ordering::Relaxed);
                continue;
            }

            if let Some(next_value) = toggle_setting(app_state, event.id().as_ref()) {
                if let Some(item) = self.toggle_items.get(event.id().as_ref()) {
                    item.set_checked(next_value);
                }
            }
        }
    }
}

fn append_toggle(
    menu: &Submenu,
    toggle_items: &mut HashMap<&'static str, CheckMenuItem>,
    id: &'static str,
    label: &str,
    checked: bool,
) -> anyhow::Result<()> {
    let item = CheckMenuItem::with_id(id, label, true, checked, None);
    menu.append(&item)?;
    toggle_items.insert(id, item);
    Ok(())
}

fn toggle_setting(app_state: &Arc<RwLock<AppState>>, id: &str) -> Option<bool> {
    macro_rules! toggle {
        ($value:expr) => {{
            $value = !$value;
            $value
        }};
    }

    let next_value = {
        let mut state = app_state.blocking_write();
        match id {
            "columns.skin" => {
                toggle!(state.config.columns.skin)
            }
            "columns.rr" => {
                toggle!(state.config.columns.rr)
            }
            "columns.earned_rr" => {
                toggle!(state.config.columns.earned_rr)
            }
            "columns.peak_rank" => {
                toggle!(state.config.columns.peak_rank)
            }
            "columns.previous_rank" => {
                toggle!(state.config.columns.previous_rank)
            }
            "columns.leaderboard" => {
                toggle!(state.config.columns.leaderboard)
            }
            "columns.headshot_percent" => {
                toggle!(state.config.columns.headshot_percent)
            }
            "columns.winrate" => {
                toggle!(state.config.columns.winrate)
            }
            "columns.kd" => {
                toggle!(state.config.columns.kd)
            }
            "columns.level" => {
                toggle!(state.config.columns.level)
            }
            "behavior.auto_show_pregame" => {
                toggle!(state.config.behavior.auto_show_pregame)
            }
            "behavior.auto_hide_ingame" => {
                toggle!(state.config.behavior.auto_hide_ingame)
            }
            "behavior.party_finder" => {
                toggle!(state.config.behavior.party_finder)
            }
            "behavior.discord_rpc" => {
                toggle!(state.config.behavior.discord_rpc)
            }
            "features.last_played" => {
                toggle!(state.config.features.last_played)
            }
            "features.auto_hide_leaderboard" => {
                toggle!(state.config.features.auto_hide_leaderboard)
            }
            "features.peak_rank_act" => {
                toggle!(state.config.features.peak_rank_act)
            }
            "features.aggregate_rank_rr" => {
                toggle!(state.config.features.aggregate_rank_rr)
            }
            "features.server_id" => {
                toggle!(state.config.features.server_id)
            }
            "features.short_ranks" => {
                toggle!(state.config.features.short_ranks)
            }
            "features.truncate_names" => {
                toggle!(state.config.features.truncate_names)
            }
            "features.truncate_ranks" => {
                toggle!(state.config.features.truncate_ranks)
            }
            "features.roman_numerals" => {
                toggle!(state.config.features.roman_numerals)
            }
            "overlay.truncate_skins" => {
                toggle!(state.config.overlay.truncate_skins)
            }
            "star.enabled" => {
                toggle!(state.config.star.enabled)
            }
            _ => return None,
        }
    };

    let config = {
        let state = app_state.blocking_read();
        state.config.clone()
    };
    if let Err(error) = config.save() {
        tracing::warn!("Failed to save tray setting '{}': {}", id, error);
    }

    Some(next_value)
}

fn load_tray_icon() -> tray_icon::Icon {
    // 16x16 solid gold star icon (minimal bitmap)
    let size = 16u32;
    let mut rgba = vec![0u8; (size * size * 4) as usize];

    // Draw a simple filled rectangle as fallback icon
    for y in 0..size {
        for x in 0..size {
            let idx = ((y * size + x) * 4) as usize;
            let dx = (x as f32 - 7.5).abs();
            let dy = (y as f32 - 7.5).abs();
            if dx + dy < 8.0 {
                rgba[idx] = 255; // R
                rgba[idx + 1] = 215; // G
                rgba[idx + 2] = 0; // B
                rgba[idx + 3] = 255; // A
            }
        }
    }

    tray_icon::Icon::from_rgba(rgba, size, size).expect("valid icon")
}
