use crate::app::AppState;
use crate::assets;
use crate::config::Config;
use crate::game::players::{normalize_overlay_weapon, OVERLAY_WEAPONS};
use crate::overlay::hotkeys::{normalize_hotkey_name, SUPPORTED_HOTKEYS};
use std::collections::HashMap;
#[cfg(target_os = "windows")]
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::{TrayIcon, TrayIconBuilder};

const HOTKEY_MENU_PREFIX: &str = "overlay.hotkey.";
const WEAPON_MENU_PREFIX: &str = "overlay.weapon.";
#[cfg(target_os = "windows")]
const TOGGLE_TERMINAL_ID: &str = "toggle_terminal";

pub struct SystemTray {
    _tray: TrayIcon,
    quit_flag: Arc<AtomicBool>,
    check_items: HashMap<&'static str, CheckMenuItem>,
    #[cfg(target_os = "windows")]
    terminal_item: MenuItem,
    #[cfg(target_os = "windows")]
    terminal_id: tray_icon::menu::MenuId,
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
        let mut check_items: HashMap<&'static str, CheckMenuItem> = HashMap::new();

        let columns_menu = Submenu::new("Columns", true);
        append_check(
            &columns_menu,
            &mut check_items,
            "columns.skin",
            "Skin",
            config.columns.skin,
        )?;
        append_check(
            &columns_menu,
            &mut check_items,
            "columns.rr",
            "RR",
            config.columns.rr,
        )?;
        append_check(
            &columns_menu,
            &mut check_items,
            "columns.earned_rr",
            "Earned RR",
            config.columns.earned_rr,
        )?;
        append_check(
            &columns_menu,
            &mut check_items,
            "columns.peak_rank",
            "Peak Rank",
            config.columns.peak_rank,
        )?;
        append_check(
            &columns_menu,
            &mut check_items,
            "columns.previous_rank",
            "Previous Rank",
            config.columns.previous_rank,
        )?;
        append_check(
            &columns_menu,
            &mut check_items,
            "columns.leaderboard",
            "Leaderboard",
            config.columns.leaderboard,
        )?;
        append_check(
            &columns_menu,
            &mut check_items,
            "columns.headshot_percent",
            "Headshot %",
            config.columns.headshot_percent,
        )?;
        append_check(
            &columns_menu,
            &mut check_items,
            "columns.winrate",
            "Winrate",
            config.columns.winrate,
        )?;
        append_check(
            &columns_menu,
            &mut check_items,
            "columns.kd",
            "K/D",
            config.columns.kd,
        )?;
        append_check(
            &columns_menu,
            &mut check_items,
            "columns.level",
            "Level",
            config.columns.level,
        )?;

        let behavior_menu = Submenu::new("Behavior", true);
        append_check(
            &behavior_menu,
            &mut check_items,
            "behavior.auto_show_pregame",
            "Auto Show Pregame",
            config.behavior.auto_show_pregame,
        )?;
        append_check(
            &behavior_menu,
            &mut check_items,
            "behavior.auto_hide_ingame",
            "Auto Hide Ingame",
            config.behavior.auto_hide_ingame,
        )?;
        append_check(
            &behavior_menu,
            &mut check_items,
            "behavior.party_finder",
            "Party Finder",
            config.behavior.party_finder,
        )?;
        append_check(
            &behavior_menu,
            &mut check_items,
            "behavior.discord_rpc",
            "Discord RPC",
            config.behavior.discord_rpc,
        )?;

        let features_menu = Submenu::new("Features", true);
        append_check(
            &features_menu,
            &mut check_items,
            "features.last_played",
            "Last Played",
            config.features.last_played,
        )?;
        append_check(
            &features_menu,
            &mut check_items,
            "features.server_id",
            "Server ID",
            config.features.server_id,
        )?;
        append_check(
            &features_menu,
            &mut check_items,
            "features.truncate_ranks",
            "Truncate Ranks",
            config.features.truncate_ranks,
        )?;
        append_check(
            &features_menu,
            &mut check_items,
            "features.roman_numerals",
            "Roman Numerals",
            config.features.roman_numerals,
        )?;
        append_check(
            &features_menu,
            &mut check_items,
            "features.truncate_names",
            "Truncate Names",
            config.features.truncate_names,
        )?;
        append_check(
            &features_menu,
            &mut check_items,
            "overlay.truncate_skins",
            "Truncate Skins",
            config.overlay.truncate_skins,
        )?;

        let overlay_menu = Submenu::new("Overlay", true);

        let hotkey_menu = Submenu::new("Hotkey", true);
        for hotkey in SUPPORTED_HOTKEYS {
            append_check(
                &hotkey_menu,
                &mut check_items,
                hotkey_menu_id(hotkey),
                hotkey,
                normalize_hotkey_name(&config.overlay.hotkey) == *hotkey,
            )?;
        }
        overlay_menu.append(&hotkey_menu)?;

        let weapon_menu = Submenu::new("Skin Weapon", true);
        for weapon in OVERLAY_WEAPONS {
            append_check(
                &weapon_menu,
                &mut check_items,
                weapon_menu_id(weapon),
                weapon,
                normalize_overlay_weapon(&config.overlay.weapon) == *weapon,
            )?;
        }
        overlay_menu.append(&weapon_menu)?;

        let star_menu = Submenu::new("Star", true);
        append_check(
            &star_menu,
            &mut check_items,
            "star.enabled",
            "Enabled",
            config.star.enabled,
        )?;

        #[cfg(target_os = "windows")]
        let terminal_item =
            MenuItem::with_id(TOGGLE_TERMINAL_ID, terminal_menu_label(&config), true, None);
        #[cfg(target_os = "windows")]
        let terminal_id = terminal_item.id().clone();
        let quit_item = MenuItem::with_id("quit", "Quit Star Client", true, None);
        let quit_id = quit_item.id().clone();
        menu.append(&columns_menu)?;
        menu.append(&behavior_menu)?;
        menu.append(&features_menu)?;
        menu.append(&overlay_menu)?;
        menu.append(&star_menu)?;
        menu.append(&PredefinedMenuItem::separator())?;
        #[cfg(target_os = "windows")]
        menu.append(&terminal_item)?;
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
            check_items,
            #[cfg(target_os = "windows")]
            terminal_item,
            #[cfg(target_os = "windows")]
            terminal_id,
            quit_id,
        })
    }

    pub fn poll_events(&self, app_state: &Arc<RwLock<AppState>>) {
        #[cfg(target_os = "windows")]
        self.sync_terminal_item_text(app_state);

        while let Ok(event) = MenuEvent::receiver().try_recv() {
            #[cfg(target_os = "windows")]
            if event.id() == &self.terminal_id {
                self.handle_terminal_restart(app_state);
                continue;
            }

            if event.id() == &self.quit_id {
                self.quit_flag.store(true, Ordering::Relaxed);
                continue;
            }

            if handle_setting_event(app_state, event.id().as_ref()) {
                let config = {
                    let state = app_state.blocking_read();
                    state.config.clone()
                };
                self.sync_from_config(&config);
            }
        }
    }

    fn sync_from_config(&self, config: &Config) {
        for (id, item) in &self.check_items {
            if let Some(checked) = check_state_for_id(config, id) {
                item.set_checked(checked);
            }
        }
    }

    #[cfg(target_os = "windows")]
    fn sync_terminal_item_text(&self, app_state: &Arc<RwLock<AppState>>) {
        let config = {
            let state = app_state.blocking_read();
            state.config.clone()
        };
        self.terminal_item.set_text(terminal_menu_label(&config));
    }

    #[cfg(target_os = "windows")]
    fn handle_terminal_restart(&self, app_state: &Arc<RwLock<AppState>>) {
        let (previous, updated_config) = {
            let mut state = app_state.blocking_write();
            let previous = state.config.behavior.launch_without_terminal;
            state.config.behavior.launch_without_terminal = !previous;
            (previous, state.config.clone())
        };

        if let Err(error) = updated_config.save() {
            tracing::warn!("Failed to save terminal launch setting: {}", error);
            self.restore_terminal_launch_setting(app_state, previous);
            self.sync_terminal_item_text(app_state);
            return;
        }

        if let Err(error) =
            relaunch_current_process(updated_config.behavior.launch_without_terminal)
        {
            tracing::warn!(
                "Failed to restart app for terminal setting change: {}",
                error
            );
            self.restore_terminal_launch_setting(app_state, previous);
            self.sync_terminal_item_text(app_state);
            return;
        }

        self.quit_flag.store(true, Ordering::Relaxed);
    }

    #[cfg(target_os = "windows")]
    fn restore_terminal_launch_setting(
        &self,
        app_state: &Arc<RwLock<AppState>>,
        launch_without_terminal: bool,
    ) {
        let reverted_config = {
            let mut state = app_state.blocking_write();
            state.config.behavior.launch_without_terminal = launch_without_terminal;
            state.config.clone()
        };

        if let Err(error) = reverted_config.save() {
            tracing::warn!(
                "Failed to restore terminal launch setting after restart failure: {}",
                error
            );
        }
    }
}

fn append_check(
    menu: &Submenu,
    check_items: &mut HashMap<&'static str, CheckMenuItem>,
    id: &'static str,
    label: &str,
    checked: bool,
) -> anyhow::Result<()> {
    let item = CheckMenuItem::with_id(id, label, true, checked, None);
    menu.append(&item)?;
    check_items.insert(id, item);
    Ok(())
}

fn hotkey_menu_id(hotkey: &str) -> &'static str {
    match hotkey {
        "F1" => "overlay.hotkey.F1",
        "F2" => "overlay.hotkey.F2",
        "F3" => "overlay.hotkey.F3",
        "F4" => "overlay.hotkey.F4",
        "F5" => "overlay.hotkey.F5",
        "F6" => "overlay.hotkey.F6",
        "F7" => "overlay.hotkey.F7",
        "F8" => "overlay.hotkey.F8",
        "F9" => "overlay.hotkey.F9",
        "F10" => "overlay.hotkey.F10",
        "F11" => "overlay.hotkey.F11",
        "F12" => "overlay.hotkey.F12",
        "Insert" => "overlay.hotkey.Insert",
        "Home" => "overlay.hotkey.Home",
        "End" => "overlay.hotkey.End",
        "PageUp" => "overlay.hotkey.PageUp",
        "PageDown" => "overlay.hotkey.PageDown",
        _ => "overlay.hotkey.F2",
    }
}

fn weapon_menu_id(weapon: &str) -> &'static str {
    match weapon {
        "Vandal" => "overlay.weapon.Vandal",
        "Phantom" => "overlay.weapon.Phantom",
        "Operator" => "overlay.weapon.Operator",
        "Sheriff" => "overlay.weapon.Sheriff",
        "Spectre" => "overlay.weapon.Spectre",
        "Classic" => "overlay.weapon.Classic",
        _ => "overlay.weapon.Vandal",
    }
}

fn handle_setting_event(app_state: &Arc<RwLock<AppState>>, id: &str) -> bool {
    let mut changed = false;
    let handled = {
        let mut state = app_state.blocking_write();
        match id {
            "columns.skin" => {
                state.config.columns.skin = !state.config.columns.skin;
                changed = true;
                true
            }
            "columns.rr" => {
                state.config.columns.rr = !state.config.columns.rr;
                changed = true;
                true
            }
            "columns.earned_rr" => {
                state.config.columns.earned_rr = !state.config.columns.earned_rr;
                changed = true;
                true
            }
            "columns.peak_rank" => {
                state.config.columns.peak_rank = !state.config.columns.peak_rank;
                changed = true;
                true
            }
            "columns.previous_rank" => {
                state.config.columns.previous_rank = !state.config.columns.previous_rank;
                changed = true;
                true
            }
            "columns.leaderboard" => {
                state.config.columns.leaderboard = !state.config.columns.leaderboard;
                changed = true;
                true
            }
            "columns.headshot_percent" => {
                state.config.columns.headshot_percent = !state.config.columns.headshot_percent;
                changed = true;
                true
            }
            "columns.winrate" => {
                state.config.columns.winrate = !state.config.columns.winrate;
                changed = true;
                true
            }
            "columns.kd" => {
                state.config.columns.kd = !state.config.columns.kd;
                changed = true;
                true
            }
            "columns.level" => {
                state.config.columns.level = !state.config.columns.level;
                changed = true;
                true
            }
            "behavior.auto_show_pregame" => {
                state.config.behavior.auto_show_pregame = !state.config.behavior.auto_show_pregame;
                changed = true;
                true
            }
            "behavior.auto_hide_ingame" => {
                state.config.behavior.auto_hide_ingame = !state.config.behavior.auto_hide_ingame;
                changed = true;
                true
            }
            "behavior.party_finder" => {
                state.config.behavior.party_finder = !state.config.behavior.party_finder;
                changed = true;
                true
            }
            "behavior.discord_rpc" => {
                state.config.behavior.discord_rpc = !state.config.behavior.discord_rpc;
                changed = true;
                true
            }
            "features.last_played" => {
                state.config.features.last_played = !state.config.features.last_played;
                changed = true;
                true
            }
            "features.server_id" => {
                state.config.features.server_id = !state.config.features.server_id;
                changed = true;
                true
            }
            "features.truncate_names" => {
                state.config.features.truncate_names = !state.config.features.truncate_names;
                changed = true;
                true
            }
            "features.truncate_ranks" => {
                state.config.features.truncate_ranks = !state.config.features.truncate_ranks;
                changed = true;
                true
            }
            "features.roman_numerals" => {
                state.config.features.roman_numerals = !state.config.features.roman_numerals;
                changed = true;
                true
            }
            "overlay.truncate_skins" => {
                state.config.overlay.truncate_skins = !state.config.overlay.truncate_skins;
                changed = true;
                true
            }
            "star.enabled" => {
                state.config.star.enabled = !state.config.star.enabled;
                changed = true;
                true
            }
            _ => {
                if let Some(hotkey) = id.strip_prefix(HOTKEY_MENU_PREFIX) {
                    let normalized = normalize_hotkey_name(hotkey);
                    changed = state.config.overlay.hotkey != normalized;
                    state.config.overlay.hotkey = normalized.to_string();
                    true
                } else if let Some(weapon) = id.strip_prefix(WEAPON_MENU_PREFIX) {
                    let normalized = normalize_overlay_weapon(weapon);
                    changed = state.config.overlay.weapon != normalized;
                    state.config.overlay.weapon = normalized.to_string();
                    true
                } else {
                    false
                }
            }
        }
    };

    if !handled {
        return false;
    }

    if changed {
        let config = {
            let state = app_state.blocking_read();
            state.config.clone()
        };
        if let Err(error) = config.save() {
            tracing::warn!("Failed to save tray setting '{}': {}", id, error);
        }
    }

    true
}

fn check_state_for_id(config: &Config, id: &str) -> Option<bool> {
    Some(match id {
        "columns.skin" => config.columns.skin,
        "columns.rr" => config.columns.rr,
        "columns.earned_rr" => config.columns.earned_rr,
        "columns.peak_rank" => config.columns.peak_rank,
        "columns.previous_rank" => config.columns.previous_rank,
        "columns.leaderboard" => config.columns.leaderboard,
        "columns.headshot_percent" => config.columns.headshot_percent,
        "columns.winrate" => config.columns.winrate,
        "columns.kd" => config.columns.kd,
        "columns.level" => config.columns.level,
        "behavior.auto_show_pregame" => config.behavior.auto_show_pregame,
        "behavior.auto_hide_ingame" => config.behavior.auto_hide_ingame,
        "behavior.party_finder" => config.behavior.party_finder,
        "behavior.discord_rpc" => config.behavior.discord_rpc,
        "features.last_played" => config.features.last_played,
        "features.server_id" => config.features.server_id,
        "features.truncate_names" => config.features.truncate_names,
        "features.truncate_ranks" => config.features.truncate_ranks,
        "features.roman_numerals" => config.features.roman_numerals,
        "overlay.truncate_skins" => config.overlay.truncate_skins,
        "star.enabled" => config.star.enabled,
        _ => {
            if let Some(hotkey) = id.strip_prefix(HOTKEY_MENU_PREFIX) {
                normalize_hotkey_name(&config.overlay.hotkey) == normalize_hotkey_name(hotkey)
            } else if let Some(weapon) = id.strip_prefix(WEAPON_MENU_PREFIX) {
                normalize_overlay_weapon(&config.overlay.weapon) == normalize_overlay_weapon(weapon)
            } else {
                return None;
            }
        }
    })
}

fn load_tray_icon() -> tray_icon::Icon {
    if let Ok((rgba, width, height)) = assets::tray_icon_rgba(32) {
        if let Ok(icon) = tray_icon::Icon::from_rgba(rgba, width, height) {
            return icon;
        }
    }

    let size = 16u32;
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let idx = ((y * size + x) * 4) as usize;
            let dx = (x as f32 - 7.5).abs();
            let dy = (y as f32 - 7.5).abs();
            if dx + dy < 8.0 {
                rgba[idx] = 255;
                rgba[idx + 1] = 215;
                rgba[idx + 2] = 0;
                rgba[idx + 3] = 255;
            }
        }
    }

    tray_icon::Icon::from_rgba(rgba, size, size).expect("valid icon")
}

#[cfg(target_os = "windows")]
fn terminal_menu_label(config: &Config) -> &'static str {
    if config.behavior.launch_without_terminal {
        "Show Terminal (Restart Required)"
    } else {
        "Hide Terminal (Restart Required)"
    }
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
