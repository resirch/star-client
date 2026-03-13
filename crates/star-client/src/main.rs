mod app;
mod assets;
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

const STARTUP_RETRY_DELAY: std::time::Duration = std::time::Duration::from_secs(2);
#[cfg(target_os = "windows")]
const DETACHED_LAUNCH_ENV: &str = "STAR_CLIENT_DETACHED";

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

    #[cfg(target_os = "windows")]
    if relaunch_without_terminal_if_needed(&config) {
        return;
    }

    #[cfg(target_os = "windows")]
    apply_terminal_launch_preference(&config);

    let quit_flag = Arc::new(AtomicBool::new(false));
    let app_state = Arc::new(RwLock::new(AppState::new(config.clone())));

    let tray = tray::SystemTray::new(Arc::clone(&app_state), Arc::clone(&quit_flag)).ok();

    let hotkey_mgr = HotkeyManager::new();
    let app_state_hotkey = Arc::clone(&app_state);
    hotkey_mgr.start(move || {
        app_state_hotkey
            .blocking_read()
            .config
            .overlay
            .hotkey
            .clone()
    });
    let key_held = hotkey_mgr.key_held();

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
            run_background_loop(app_state_bg, config_bg, quit_flag_bg).await;
        });
    });

    run_overlay(app_state, quit_flag, key_held, tray);
}

#[cfg(target_os = "windows")]
fn relaunch_without_terminal_if_needed(config: &Config) -> bool {
    use std::os::windows::process::CommandExt;
    use std::process::Stdio;
    use windows_sys::Win32::System::Console::GetConsoleWindow;
    use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;

    if !config.behavior.launch_without_terminal
        || std::env::var_os(DETACHED_LAUNCH_ENV).is_some()
    {
        return false;
    }

    let console = unsafe { GetConsoleWindow() };
    if console.is_null() {
        return false;
    }

    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(error) => {
            tracing::warn!("Failed to determine current executable for relaunch: {}", error);
            return false;
        }
    };

    let mut command = std::process::Command::new(exe);
    command
        .args(std::env::args_os().skip(1))
        .env(DETACHED_LAUNCH_ENV, "1")
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    match command.spawn() {
        Ok(_) => true,
        Err(error) => {
            tracing::warn!("Failed to relaunch without terminal: {}", error);
            false
        }
    }
}

#[cfg(target_os = "windows")]
fn apply_terminal_launch_preference(config: &Config) {
    use windows_sys::Win32::System::Console::{GetConsoleProcessList, GetConsoleWindow};
    use windows_sys::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE, SW_SHOW};

    let console = unsafe { GetConsoleWindow() };
    if console.is_null() {
        return;
    }

    let mut process_ids = [0u32; 8];
    let attached_processes =
        unsafe { GetConsoleProcessList(process_ids.as_mut_ptr(), process_ids.len() as u32) };
    if attached_processes > 1 {
        return;
    }

    unsafe {
        ShowWindow(
            console,
            if config.behavior.launch_without_terminal {
                SW_HIDE
            } else {
                SW_SHOW
            },
        );
    }
}

async fn run_background_loop(
    app_state: Arc<RwLock<AppState>>,
    config: Config,
    quit_flag: Arc<AtomicBool>,
) {
    loop {
        if quit_flag.load(Ordering::Relaxed) {
            return;
        }

        let lockfile_data = tokio::task::spawn_blocking(lockfile::wait_for_lockfile)
            .await
            .expect("lockfile task");

        if quit_flag.load(Ordering::Relaxed) {
            return;
        }

        let riot_auth = match auth::authenticate(&lockfile_data).await {
            Ok(auth) => auth,
            Err(e) => {
                tracing::warn!("Authentication failed, retrying: {}", e);
                tokio::time::sleep(STARTUP_RETRY_DELAY).await;
                continue;
            }
        };

        tracing::info!(
            "Authenticated as {} (region: {}, shard: {})",
            &riot_auth.puuid[..8],
            riot_auth.region,
            riot_auth.shard
        );

        {
            let mut state = app_state.write().await;
            state.local_puuid = riot_auth.puuid.clone();
        }

        let mut api_client = RiotApiClient::new(riot_auth.clone()).expect("API client");
        if let Err(e) = api_client.fetch_client_version().await {
            tracing::warn!("Could not fetch client version: {}", e);
        }
        let api = Arc::new(RwLock::new(api_client));

        let star_client = Arc::new(StarClient::new(&config.star.backend_url));
        if config.star.enabled {
            if let Err(e) = star_client.register(&riot_auth.puuid).await {
                tracing::warn!("Star registration failed (backend may be offline): {}", e);
            }
            star_client.start_heartbeat_loop();
        }

        app::run_data_loop(
            Arc::clone(&app_state),
            api,
            star_client,
            Arc::clone(&quit_flag),
        )
        .await;

        if quit_flag.load(Ordering::Relaxed) {
            return;
        }

        tracing::warn!("Data loop exited unexpectedly, restarting session bootstrap");
        tokio::time::sleep(STARTUP_RETRY_DELAY).await;
    }
}

fn run_overlay(
    app_state: Arc<RwLock<AppState>>,
    quit_flag: Arc<AtomicBool>,
    key_held: Arc<AtomicBool>,
    tray: Option<tray::SystemTray>,
) {
    use egui_overlay::EguiOverlay;

    struct StarOverlay {
        app_state: Arc<RwLock<AppState>>,
        quit_flag: Arc<AtomicBool>,
        key_held: Arc<AtomicBool>,
        tray: Option<tray::SystemTray>,
        initialized: bool,
        shown: bool,
        topmost_active: bool,
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
                overlay::theme::configure_fonts(egui_context);
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

            if let Some(tray) = &self.tray {
                tray.poll_events(&self.app_state);
            }

            let hotkey_active = self.key_held.load(Ordering::Relaxed);
            let mut should_be_topmost = false;

            if let Ok(state) = self.app_state.try_read() {
                let visible = state.auto_visible || hotkey_active;
                should_be_topmost = visible && valorant_is_focused();
                if should_be_topmost {
                    overlay::ui::render_overlay(
                        egui_context,
                        &state.game_state,
                        &state.players,
                        state.match_context.as_ref(),
                        &state.local_puuid,
                        &state.config,
                    );
                }
            }

            if should_be_topmost != self.topmost_active {
                set_overlay_topmost(glfw_backend, should_be_topmost);
                self.topmost_active = should_be_topmost;
            }

            egui_context.request_repaint_after(std::time::Duration::from_millis(16));
        }
    }

    start_overlay(StarOverlay {
        app_state,
        quit_flag,
        key_held,
        tray,
        initialized: false,
        shown: false,
        topmost_active: false,
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
    glfw_backend.window.set_floating(false);

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
            }
        }

        tracing::info!(
            "Window initialized: {}x{}, hidden from taskbar",
            screen_w,
            screen_h
        );
    }

    glfw_backend.set_passthrough(true);
}

fn set_overlay_topmost(
    glfw_backend: &mut egui_overlay::egui_window_glfw_passthrough::GlfwBackend,
    topmost: bool,
) {
    glfw_backend.window.set_floating(topmost);

    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            SetWindowPos, HWND_NOTOPMOST, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
        };

        let hwnd = glfw_backend.window.get_win32_window();
        if !hwnd.is_null() {
            unsafe {
                SetWindowPos(
                    hwnd,
                    if topmost {
                        HWND_TOPMOST
                    } else {
                        HWND_NOTOPMOST
                    },
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                );
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn valorant_is_focused() -> bool {
    use std::path::Path;
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowThreadProcessId,
    };

    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return false;
        }

        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == 0 {
            return false;
        }

        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process.is_null() {
            return false;
        }

        let mut buffer = vec![0u16; 1024];
        let mut len = buffer.len() as u32;
        let ok = QueryFullProcessImageNameW(process, 0, buffer.as_mut_ptr(), &mut len);
        CloseHandle(process);

        if ok == 0 || len == 0 {
            return false;
        }

        let exe_path = String::from_utf16_lossy(&buffer[..len as usize]);
        Path::new(&exe_path)
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.eq_ignore_ascii_case("VALORANT-Win64-Shipping.exe"))
            .unwrap_or(false)
    }
}

#[cfg(not(target_os = "windows"))]
fn valorant_is_focused() -> bool {
    true
}
