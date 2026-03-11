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

    let _tray = tray::SystemTray::new(Arc::clone(&quit_flag)).ok();

    let hotkey_mgr = HotkeyManager::new();
    hotkey_mgr.start(&config.overlay.hotkey);
    let key_held = hotkey_mgr.key_held();

    let app_state = Arc::new(RwLock::new(AppState::new(config.clone())));

    let app_state_bg = Arc::clone(&app_state);
    let quit_flag_bg = Arc::clone(&quit_flag);
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

            let mut api_client = RiotApiClient::new(riot_auth.clone()).expect("API client");
            if let Err(e) = api_client.fetch_client_version().await {
                tracing::warn!("Could not fetch client version: {}", e);
            }
            let api = Arc::new(RwLock::new(api_client));

            let star_client = Arc::new(StarClient::new(&config_bg.star.backend_url));
            if config_bg.star.enabled {
                if let Err(e) = star_client.register(&riot_auth.puuid).await {
                    tracing::warn!("Star registration failed (backend may be offline): {}", e);
                }
                star_client.start_heartbeat_loop();
            }

            app::run_data_loop(app_state_bg, api, star_client, quit_flag_bg).await;
        });
    });

    run_overlay(app_state, quit_flag, key_held);
}

fn run_overlay(
    app_state: Arc<RwLock<AppState>>,
    quit_flag: Arc<AtomicBool>,
    key_held: Arc<AtomicBool>,
) {
    use egui_overlay::EguiOverlay;

    struct StarOverlay {
        app_state: Arc<RwLock<AppState>>,
        quit_flag: Arc<AtomicBool>,
        key_held: Arc<AtomicBool>,
        initialized: bool,
        shown: bool,
    }

    impl EguiOverlay for StarOverlay {
        fn run(
            &mut self,
            egui_context: &egui::Context,
            default_gfx_backend: &mut egui_overlay::egui_render_three_d::ThreeDBackend,
            glfw_backend: &mut egui_overlay::egui_window_glfw_passthrough::GlfwBackend,
        ) -> Option<(egui::PlatformOutput, std::time::Duration)> {
            if self.quit_flag.load(Ordering::Relaxed) {
                glfw_backend.window.set_should_close(true);
                return None;
            }

            if !self.initialized {
                self.initialized = true;
                init_window(glfw_backend);
            }

            unsafe {
                use egui_overlay::egui_render_three_d::glow::HasContext;
                default_gfx_backend
                    .glow_backend
                    .glow_context
                    .clear_color(0.0, 0.0, 0.0, 0.0);
            }

            glfw_backend.set_passthrough(true);

            let input = glfw_backend.take_raw_input();
            default_gfx_backend.prepare_frame(|| {
                let latest_size = glfw_backend.window.get_framebuffer_size();
                [latest_size.0 as _, latest_size.1 as _]
            });

            egui_context.begin_pass(input);
            self.gui_run(egui_context, default_gfx_backend, glfw_backend);

            let egui::FullOutput {
                platform_output,
                textures_delta,
                shapes,
                pixels_per_point,
                viewport_output,
            } = egui_context.end_pass();
            let meshes = egui_context.tessellate(shapes, pixels_per_point);
            let repaint_after = viewport_output
                .into_iter()
                .map(|f| f.1.repaint_delay)
                .collect::<Vec<std::time::Duration>>()[0];

            default_gfx_backend.render_egui(
                meshes,
                textures_delta,
                glfw_backend.window_size_logical,
            );

            use egui_overlay::egui_window_glfw_passthrough::glfw::Context;
            glfw_backend.window.swap_buffers();

            if !self.shown {
                glfw_backend.window.show();
                self.shown = true;
            }

            Some((platform_output, repaint_after))
        }

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

            let hotkey_active = self.key_held.load(Ordering::Relaxed);

            if let Ok(state) = self.app_state.try_read() {
                let visible = state.auto_visible || hotkey_active;
                if visible {
                    overlay::ui::render_overlay(
                        egui_context,
                        &state.game_state,
                        &state.players,
                        &state.config.columns,
                    );
                }
            }

            egui_context.request_repaint_after(std::time::Duration::from_millis(16));
        }
    }

    start_overlay(StarOverlay {
        app_state,
        quit_flag,
        key_held,
        initialized: false,
        shown: false,
    });
}

fn start_overlay<T: egui_overlay::EguiOverlay + 'static>(user_data: T) {
    use egui_overlay::egui_render_three_d::ThreeDBackend;
    use egui_overlay::egui_window_glfw_passthrough::{
        glfw::{ClientApiHint, WindowHint},
        GlfwBackend, GlfwConfig,
    };
    use egui_overlay::OverlayApp;

    let mut glfw_backend = GlfwBackend::new(GlfwConfig {
        glfw_callback: Box::new(|gtx| {
            gtx.window_hint(WindowHint::ScaleToMonitor(true));
            gtx.window_hint(WindowHint::Decorated(false));
            gtx.window_hint(WindowHint::Floating(true));
            gtx.window_hint(WindowHint::Focused(false));
            gtx.window_hint(WindowHint::FocusOnShow(false));
            gtx.window_hint(WindowHint::MousePassthrough(true));
            gtx.window_hint(WindowHint::Visible(false));
            gtx.window_hint(WindowHint::ClientApi(ClientApiHint::OpenGl));
        }),
        opengl_window: Some(true),
        transparent_window: Some(true),
        ..Default::default()
    });

    glfw_backend.window.set_floating(true);
    glfw_backend.window.set_decorated(false);
    glfw_backend.window.set_focus_on_show(false);
    glfw_backend.window.set_mouse_passthrough(true);

    let latest_size = glfw_backend.window.get_framebuffer_size();
    let latest_size = [latest_size.0 as _, latest_size.1 as _];
    let default_gfx_backend = ThreeDBackend::new(
        egui_overlay::egui_render_three_d::ThreeDConfig::default(),
        |s| glfw_backend.get_proc_address(s),
        latest_size,
    );

    OverlayApp {
        user_data,
        egui_context: Default::default(),
        default_gfx_backend,
        glfw_backend,
    }
    .enter_event_loop();
}

fn init_window(glfw_backend: &mut egui_overlay::egui_window_glfw_passthrough::GlfwBackend) {
    glfw_backend.window.set_floating(true);

    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::*;

        let screen_w = unsafe { GetSystemMetrics(SM_CXSCREEN) };
        let screen_h = unsafe { GetSystemMetrics(SM_CYSCREEN) };
        glfw_backend.window.set_pos(0, 0);
        glfw_backend.set_window_size([screen_w as f32, screen_h as f32]);

        let hwnd = glfw_backend.window.get_win32_window();
        if !hwnd.is_null() {
            unsafe {
                let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
                SetWindowLongPtrW(
                    hwnd,
                    GWL_EXSTYLE,
                    (ex_style | WS_EX_TOOLWINDOW as isize | WS_EX_TRANSPARENT as isize)
                        & !(WS_EX_APPWINDOW as isize),
                );

                SetWindowPos(
                    hwnd,
                    HWND_TOPMOST,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                );
            }
        }

        tracing::info!(
            "Window initialized: {}x{}, topmost, hidden from taskbar",
            screen_w,
            screen_h
        );
    }

    glfw_backend.set_passthrough(true);
}
