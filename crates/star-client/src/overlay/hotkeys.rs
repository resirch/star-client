use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub const SUPPORTED_HOTKEYS: &[&str] = &[
    "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12", "Insert", "Home",
    "End", "PageUp", "PageDown",
];

pub struct HotkeyManager {
    key_held: Arc<AtomicBool>,
}

impl HotkeyManager {
    pub fn new() -> Self {
        Self {
            key_held: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn key_held(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.key_held)
    }

    /// Polls the hotkey state on a background thread.
    /// The flag reflects whether the key is currently held down.
    pub fn start<F>(&self, get_hotkey_name: F)
    where
        F: Fn() -> String + Send + 'static,
    {
        let flag = Arc::clone(&self.key_held);

        std::thread::spawn(move || {
            let mut hotkey_name = normalize_hotkey_name(&get_hotkey_name()).to_string();
            let mut vk = hotkey_name_to_vk(&hotkey_name);

            loop {
                std::thread::sleep(std::time::Duration::from_millis(16));

                let next_hotkey = normalize_hotkey_name(&get_hotkey_name());
                if next_hotkey != hotkey_name {
                    hotkey_name = next_hotkey.to_string();
                    vk = hotkey_name_to_vk(&hotkey_name);
                }

                #[cfg(target_os = "windows")]
                {
                    let state = unsafe {
                        windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState(vk)
                    };
                    // High bit set = key is currently held down
                    let held = (state & (1 << 15)) != 0;
                    flag.store(held, Ordering::Relaxed);
                }

                #[cfg(not(target_os = "windows"))]
                {
                    let _ = (flag.as_ref(), vk);
                }
            }
        });
    }
}

pub fn normalize_hotkey_name(name: &str) -> &'static str {
    let trimmed = name.trim();
    match trimmed.to_ascii_uppercase().as_str() {
        "INS" => return "Insert",
        "PGUP" => return "PageUp",
        "PGDN" => return "PageDown",
        _ => {}
    }
    SUPPORTED_HOTKEYS
        .iter()
        .copied()
        .find(|option| option.eq_ignore_ascii_case(trimmed))
        .unwrap_or("F2")
}

fn hotkey_name_to_vk(name: &str) -> i32 {
    match normalize_hotkey_name(name).to_ascii_uppercase().as_str() {
        "F1" => 0x70,
        "F2" => 0x71,
        "F3" => 0x72,
        "F4" => 0x73,
        "F5" => 0x74,
        "F6" => 0x75,
        "F7" => 0x76,
        "F8" => 0x77,
        "F9" => 0x78,
        "F10" => 0x79,
        "F11" => 0x7A,
        "F12" => 0x7B,
        "INSERT" => 0x2D,
        "HOME" => 0x24,
        "END" => 0x23,
        "PAGEUP" => 0x21,
        "PAGEDOWN" => 0x22,
        _ => 0x71,
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_hotkey_name;

    #[test]
    fn normalizes_supported_hotkeys() {
        assert_eq!(normalize_hotkey_name("f4"), "F4");
        assert_eq!(normalize_hotkey_name("pageup"), "PageUp");
        assert_eq!(normalize_hotkey_name("insert"), "Insert");
    }

    #[test]
    fn falls_back_to_default_hotkey() {
        assert_eq!(normalize_hotkey_name("space"), "F2");
    }
}
