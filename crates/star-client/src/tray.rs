use crate::assets;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{TrayIcon, TrayIconBuilder};

pub struct SystemTray {
    _tray: TrayIcon,
    quit_flag: Arc<AtomicBool>,
    settings_requested: Arc<AtomicBool>,
    open_settings_id: tray_icon::menu::MenuId,
    quit_id: tray_icon::menu::MenuId,
}

impl SystemTray {
    pub fn new(
        quit_flag: Arc<AtomicBool>,
        settings_requested: Arc<AtomicBool>,
    ) -> anyhow::Result<Self> {
        let menu = Menu::new();
        let open_settings_item = MenuItem::with_id("open_settings", "Open Settings", true, None);
        let open_settings_id = open_settings_item.id().clone();
        let quit_item = MenuItem::with_id("quit", "Exit", true, None);
        let quit_id = quit_item.id().clone();
        menu.append(&open_settings_item)?;
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
            settings_requested,
            open_settings_id,
            quit_id,
        })
    }

    pub fn poll_events(&self) {
        while let Ok(event) = MenuEvent::receiver().try_recv() {
            if event.id() == &self.quit_id {
                self.quit_flag.store(true, Ordering::Relaxed);
                continue;
            }

            if event.id() == &self.open_settings_id {
                self.settings_requested.store(true, Ordering::Relaxed);
            }
        }
    }
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
