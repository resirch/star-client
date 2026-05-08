use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

const DEFAULT_HOTKEY: &str = "F2";

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
            let mut hotkey_name = normalize_hotkey_name(&get_hotkey_name());
            let mut hotkey = parse_hotkey(&hotkey_name).unwrap_or_else(default_hotkey);

            loop {
                std::thread::sleep(std::time::Duration::from_millis(16));

                let next_hotkey = normalize_hotkey_name(&get_hotkey_name());
                if next_hotkey != hotkey_name {
                    hotkey_name = next_hotkey;
                    hotkey = parse_hotkey(&hotkey_name).unwrap_or_else(default_hotkey);
                }

                #[cfg(target_os = "windows")]
                {
                    let held = hotkey.is_held();
                    flag.store(held, Ordering::Relaxed);
                }

                #[cfg(not(target_os = "windows"))]
                {
                    let _ = (flag.as_ref(), &hotkey);
                }
            }
        });
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Hotkey {
    ctrl: bool,
    shift: bool,
    alt: bool,
    win: bool,
    key: HotkeyKey,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct HotkeyKey {
    label: String,
    vk: i32,
}

impl std::fmt::Display for Hotkey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut parts = Vec::new();
        if self.ctrl {
            parts.push("Ctrl");
        }
        if self.shift {
            parts.push("Shift");
        }
        if self.alt {
            parts.push("Alt");
        }
        if self.win {
            parts.push("Win");
        }
        parts.push(&self.key.label);
        write!(f, "{}", parts.join("+"))
    }
}

impl Hotkey {
    #[cfg(target_os = "windows")]
    fn is_held(&self) -> bool {
        self.key.is_held()
            && (!self.ctrl || any_vk_held(&[0x11, 0xA2, 0xA3]))
            && (!self.shift || any_vk_held(&[0x10, 0xA0, 0xA1]))
            && (!self.alt || any_vk_held(&[0x12, 0xA4, 0xA5]))
            && (!self.win || any_vk_held(&[0x5B, 0x5C]))
    }
}

impl HotkeyKey {
    #[cfg(target_os = "windows")]
    fn is_held(&self) -> bool {
        any_vk_held(&[self.vk])
    }
}

pub fn normalize_hotkey_name(name: &str) -> String {
    parse_hotkey(name)
        .unwrap_or_else(default_hotkey)
        .to_string()
}

pub fn hotkey_name_from_egui(key: egui::Key, modifiers: egui::Modifiers) -> Option<String> {
    let key = key_to_hotkey_key(key)?;
    let hotkey = Hotkey {
        ctrl: modifiers.ctrl,
        shift: modifiers.shift,
        alt: modifiers.alt,
        win: modifiers.mac_cmd,
        key,
    };
    Some(hotkey.to_string())
}

fn default_hotkey() -> Hotkey {
    Hotkey {
        ctrl: false,
        shift: false,
        alt: false,
        win: false,
        key: HotkeyKey {
            label: DEFAULT_HOTKEY.to_string(),
            vk: 0x71,
        },
    }
}

fn parse_hotkey(name: &str) -> Option<Hotkey> {
    let mut ctrl = false;
    let mut shift = false;
    let mut alt = false;
    let mut win = false;
    let mut key = None;

    for part in name.split('+').map(str::trim).filter(|part| !part.is_empty()) {
        match part.to_ascii_uppercase().as_str() {
            "CTRL" | "CONTROL" => ctrl = true,
            "SHIFT" => shift = true,
            "ALT" | "OPTION" => alt = true,
            "WIN" | "WINDOWS" | "META" | "CMD" | "COMMAND" => win = true,
            _ => {
                if key.is_some() {
                    return None;
                }
                key = parse_key(part);
            }
        }
    }

    Some(Hotkey {
        ctrl,
        shift,
        alt,
        win,
        key: key?,
    })
}

fn parse_key(name: &str) -> Option<HotkeyKey> {
    let upper = name.trim().to_ascii_uppercase();
    let normalized = match upper.as_str() {
        "INS" => "INSERT".to_string(),
        "PGUP" => "PAGEUP".to_string(),
        "PGDN" => "PAGEDOWN".to_string(),
        "ESC" | "ESCAPE" => return None,
        _ => upper,
    };

    let (label, vk) = match normalized.as_str() {
        "INSERT" => ("Insert".to_string(), 0x2D),
        "DELETE" | "DEL" => ("Delete".to_string(), 0x2E),
        "HOME" => ("Home".to_string(), 0x24),
        "END" => ("End".to_string(), 0x23),
        "PAGEUP" => ("PageUp".to_string(), 0x21),
        "PAGEDOWN" => ("PageDown".to_string(), 0x22),
        "TAB" => ("Tab".to_string(), 0x09),
        "BACKSPACE" => ("Backspace".to_string(), 0x08),
        "ENTER" | "RETURN" => ("Enter".to_string(), 0x0D),
        "SPACE" => ("Space".to_string(), 0x20),
        "ARROWUP" | "UP" => ("ArrowUp".to_string(), 0x26),
        "ARROWDOWN" | "DOWN" => ("ArrowDown".to_string(), 0x28),
        "ARROWLEFT" | "LEFT" => ("ArrowLeft".to_string(), 0x25),
        "ARROWRIGHT" | "RIGHT" => ("ArrowRight".to_string(), 0x27),
        _ => {
            if let Some(number) = normalized.strip_prefix('F') {
                let number = number.parse::<i32>().ok()?;
                if (1..=24).contains(&number) {
                    (format!("F{number}"), 0x6F + number)
                } else {
                    return None;
                }
            } else if normalized.len() == 1 {
                let byte = normalized.as_bytes()[0];
                if byte.is_ascii_uppercase() || byte.is_ascii_digit() {
                    (normalized, byte as i32)
                } else {
                    return None;
                }
            } else {
                return None;
            }
        }
    };

    Some(HotkeyKey { label, vk })
}

fn key_to_hotkey_key(key: egui::Key) -> Option<HotkeyKey> {
    use egui::Key;

    let label = match key {
        Key::Escape => return None,
        Key::ArrowDown => "ArrowDown",
        Key::ArrowLeft => "ArrowLeft",
        Key::ArrowRight => "ArrowRight",
        Key::ArrowUp => "ArrowUp",
        Key::Tab => "Tab",
        Key::Backspace => "Backspace",
        Key::Enter => "Enter",
        Key::Space => "Space",
        Key::Insert => "Insert",
        Key::Delete => "Delete",
        Key::Home => "Home",
        Key::End => "End",
        Key::PageUp => "PageUp",
        Key::PageDown => "PageDown",
        Key::Num0 => "0",
        Key::Num1 => "1",
        Key::Num2 => "2",
        Key::Num3 => "3",
        Key::Num4 => "4",
        Key::Num5 => "5",
        Key::Num6 => "6",
        Key::Num7 => "7",
        Key::Num8 => "8",
        Key::Num9 => "9",
        Key::A => "A",
        Key::B => "B",
        Key::C => "C",
        Key::D => "D",
        Key::E => "E",
        Key::F => "F",
        Key::G => "G",
        Key::H => "H",
        Key::I => "I",
        Key::J => "J",
        Key::K => "K",
        Key::L => "L",
        Key::M => "M",
        Key::N => "N",
        Key::O => "O",
        Key::P => "P",
        Key::Q => "Q",
        Key::R => "R",
        Key::S => "S",
        Key::T => "T",
        Key::U => "U",
        Key::V => "V",
        Key::W => "W",
        Key::X => "X",
        Key::Y => "Y",
        Key::Z => "Z",
        Key::F1 => "F1",
        Key::F2 => "F2",
        Key::F3 => "F3",
        Key::F4 => "F4",
        Key::F5 => "F5",
        Key::F6 => "F6",
        Key::F7 => "F7",
        Key::F8 => "F8",
        Key::F9 => "F9",
        Key::F10 => "F10",
        Key::F11 => "F11",
        Key::F12 => "F12",
        Key::F13 => "F13",
        Key::F14 => "F14",
        Key::F15 => "F15",
        Key::F16 => "F16",
        Key::F17 => "F17",
        Key::F18 => "F18",
        Key::F19 => "F19",
        Key::F20 => "F20",
        Key::F21 => "F21",
        Key::F22 => "F22",
        Key::F23 => "F23",
        Key::F24 => "F24",
        _ => return None,
    };

    parse_key(label)
}

#[cfg(target_os = "windows")]
fn any_vk_held(vks: &[i32]) -> bool {
    vks.iter().any(|vk| {
        let state = unsafe { windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState(*vk) };
        (state & (1 << 15)) != 0
    })
}

#[cfg(test)]
mod tests {
    use super::normalize_hotkey_name;

    #[test]
    fn normalizes_supported_hotkeys() {
        assert_eq!(normalize_hotkey_name("f4"), "F4");
        assert_eq!(normalize_hotkey_name("pageup"), "PageUp");
        assert_eq!(normalize_hotkey_name("insert"), "Insert");
        assert_eq!(normalize_hotkey_name("ctrl + shift + f4"), "Ctrl+Shift+F4");
        assert_eq!(normalize_hotkey_name("alt+a"), "Alt+A");
    }

    #[test]
    fn falls_back_to_default_hotkey() {
        assert_eq!(normalize_hotkey_name("unknown"), "F2");
        assert_eq!(normalize_hotkey_name("esc"), "F2");
    }
}
