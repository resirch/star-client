use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

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
    pub fn start(&self, hotkey_name: &str) {
        let flag = Arc::clone(&self.key_held);
        let vk = hotkey_name_to_vk(hotkey_name);

        std::thread::spawn(move || {
            loop {
                std::thread::sleep(std::time::Duration::from_millis(16));

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

fn hotkey_name_to_vk(name: &str) -> i32 {
    match name.to_uppercase().as_str() {
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
        "INSERT" | "INS" => 0x2D,
        "HOME" => 0x24,
        "END" => 0x23,
        "PAGEUP" => 0x21,
        "PAGEDOWN" => 0x22,
        _ => 0x71,
    }
}
