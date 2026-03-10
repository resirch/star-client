mod app;
mod config;
mod discord;
mod game;
mod overlay;
mod riot;
mod star;
mod stats;
mod tray;

use app::AppState;
use config::Config;
use overlay::hotkeys::HotkeyManager;
use riot::{api::RiotApiClient, auth, lockfile};
use star::client::StarClient;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "star_client=info,warn".into()),
        )
        .init();

    tracing::info!("Star Client v{}", env!("CARGO_PKG_VERSION"));

    let config = Config::load().unwrap_or_else(|e| {
        tracing::error!("Failed to load config: {}", e);
        Config::default()
    });

    let quit_flag = Arc::new(AtomicBool::new(false));

    // System tray (must be created on main thread for Windows)
    let _tray = tray::SystemTray::new(Arc::clone(&quit_flag)).ok();

    // Hotkey manager
    let hotkey_mgr = HotkeyManager::new();
    hotkey_mgr.start(&config.overlay.hotkey);
    let toggle_flag = hotkey_mgr.toggle_flag();

    // App state shared between data loop and overlay renderer
    let app_state = Arc::new(RwLock::new(AppState::new(config.clone())));

    // Start the async runtime for data fetching in a background thread
    let app_state_bg = Arc::clone(&app_state);
    let quit_flag_bg = Arc::clone(&quit_flag);
    let toggle_flag_bg = Arc::clone(&toggle_flag);
    let config_bg = config.clone();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .worker_threads(2)
            .build()
            .expect("tokio runtime");

        rt.block_on(async move {
            let lockfile_data = tokio::task::spawn_blocking(lockfile::wait_for_lockfile)
                .await
                .expect("lockfile task");

            let riot_auth = match auth::authenticate(&lockfile_data).await {
                Ok(a) => a,
                Err(e) => {
                    tracing::error!("Authentication failed: {}", e);
                    return;
                }
            };

            tracing::info!(
                "Authenticated as {} (region: {}, shard: {})",
                &riot_auth.puuid[..8],
                riot_auth.region,
                riot_auth.shard
            );

            let api = Arc::new(RwLock::new(
                RiotApiClient::new(riot_auth.clone()).expect("API client"),
            ));

            let star_client = Arc::new(StarClient::new(&config_bg.star.backend_url));
            if config_bg.star.enabled {
                if let Err(e) = star_client.register(&riot_auth.puuid).await {
                    tracing::warn!("Star registration failed (backend may be offline): {}", e);
                }
                star_client.start_heartbeat_loop();
            }

            app::run_data_loop(
                app_state_bg,
                api,
                star_client,
                toggle_flag_bg,
                quit_flag_bg,
            )
            .await;
        });
    });

    run_overlay(app_state, quit_flag);
}

fn run_overlay(app_state: Arc<RwLock<AppState>>, quit_flag: Arc<AtomicBool>) {
    use egui_overlay::EguiOverlay;

    struct StarOverlay {
        app_state: Arc<RwLock<AppState>>,
        quit_flag: Arc<AtomicBool>,
        initialized: bool,
    }

    impl EguiOverlay for StarOverlay {
        fn gui_run(
            &mut self,
            egui_context: &egui::Context,
            _default_gfx_backend: &mut egui_overlay::egui_render_three_d::ThreeDBackend,
            glfw_backend: &mut egui_overlay::egui_window_glfw_passthrough::GlfwBackend,
        ) {
            if self.quit_flag.load(Ordering::Relaxed) {
                glfw_backend.window.set_should_close(true);
                return;
            }

            if !self.initialized {
                self.initialized = true;
                glfw_backend.window.set_floating(true);
                apply_window_flags(glfw_backend);
            }

            if let Ok(state) = self.app_state.try_read() {
                state.overlay.render(
                    egui_context,
                    &state.game_state,
                    &state.players,
                    &state.config.columns,
                );
            }

            egui_context.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }

    egui_overlay::start(StarOverlay {
        app_state,
        quit_flag,
        initialized: false,
    });
}

/// Sets the overlay window as TOPMOST and hides it from the taskbar.
fn apply_window_flags(
    glfw_backend: &mut egui_overlay::egui_window_glfw_passthrough::GlfwBackend,
) {
    #[cfg(target_os = "windows")]
    {
        let hwnd = glfw_backend.window.get_win32_window();
        if !hwnd.is_null() {
            use windows_sys::Win32::UI::WindowsAndMessaging::*;
            unsafe {
                let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
                SetWindowLongPtrW(
                    hwnd,
                    GWL_EXSTYLE,
                    (ex_style | WS_EX_TOOLWINDOW as isize) & !(WS_EX_APPWINDOW as isize),
                );

                SetWindowPos(
                    hwnd,
                    HWND_TOPMOST,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_FRAMECHANGED | SWP_NOACTIVATE,
                );
            }
            tracing::info!("Window set to topmost + hidden from taskbar");
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = glfw_backend;
    }
}
