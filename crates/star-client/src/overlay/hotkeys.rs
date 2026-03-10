use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub struct HotkeyManager {
    toggle_flag: Arc<AtomicBool>,
}

impl HotkeyManager {
    pub fn new() -> Self {
        Self {
            toggle_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn toggle_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.toggle_flag)
    }

    /// Starts listening for the overlay toggle hotkey on a background thread.
    /// Uses Windows raw input to avoid interfering with the game.
    pub fn start(&self, hotkey_name: &str) {
        let flag = Arc::clone(&self.toggle_flag);
        let vk = hotkey_name_to_vk(hotkey_name);

        std::thread::spawn(move || {
            loop {
                std::thread::sleep(std::time::Duration::from_millis(50));

                #[cfg(target_os = "windows")]
                {
                    let pressed = unsafe { windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState(vk) };
                    if pressed as u16 & 0x0001 != 0 {
                        flag.store(true, Ordering::Relaxed);
                    }
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
        _ => 0x71, // Default F2
    }
}
