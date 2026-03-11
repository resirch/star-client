use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tray_icon::menu::{Menu, MenuEvent, MenuItem};
use tray_icon::{TrayIcon, TrayIconBuilder};

pub struct SystemTray {
    _tray: TrayIcon,
    quit_flag: Arc<AtomicBool>,
}

impl SystemTray {
    pub fn new(quit_flag: Arc<AtomicBool>) -> anyhow::Result<Self> {
        let menu = Menu::new();
        let quit_item = MenuItem::new("Quit Star Client", true, None);
        let quit_id = quit_item.id().clone();
        menu.append(&quit_item)?;

        let icon = load_tray_icon();

        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("Star Client")
            .with_icon(icon)
            .build()?;

        let quit_flag_clone = Arc::clone(&quit_flag);
        std::thread::spawn(move || loop {
            if let Ok(event) = MenuEvent::receiver().try_recv() {
                if event.id() == &quit_id {
                    quit_flag_clone.store(true, Ordering::Relaxed);
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        });

        Ok(Self {
            _tray: tray,
            quit_flag,
        })
    }

    pub fn should_quit(&self) -> bool {
        self.quit_flag.load(Ordering::Relaxed)
    }
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
