mod app;
mod assets;
mod config;
mod discord;
mod game;
mod overlay;
mod riot;
mod settings;
mod star;
mod stats;
mod tray;
mod updater;

use app::AppState;
use config::Config;
use overlay::hotkeys::HotkeyManager;
use riot::{api::RiotApiClient, auth, lockfile};
use star::client::StarClient;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;

const STARTUP_RETRY_DELAY: std::time::Duration = std::time::Duration::from_secs(2);
const DATA_LOOP_WATCHDOG_INTERVAL: std::time::Duration = std::time::Duration::from_secs(5);
const DATA_LOOP_STALL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);
const SETTINGS_SCROLL_MULTIPLIER: f32 = 64.0;
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

    let mut config = Config::load().unwrap_or_else(|e| {
        tracing::error!("Failed to load config: {}", e);
        Config::default()
    });
    config.star.enabled = true;

    #[cfg(target_os = "windows")]
    if relaunch_without_terminal_if_needed(&config) {
        return;
    }

    #[cfg(target_os = "windows")]
    apply_terminal_launch_preference(&config);

    let quit_flag = Arc::new(AtomicBool::new(false));
    let settings_requested = Arc::new(AtomicBool::new(false));
    let app_state = Arc::new(RwLock::new(AppState::new(config.clone())));

    let tray = tray::SystemTray::new(Arc::clone(&quit_flag), Arc::clone(&settings_requested)).ok();

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

    run_overlay(app_state, quit_flag, key_held, tray, settings_requested);
}

#[cfg(target_os = "windows")]
fn relaunch_without_terminal_if_needed(config: &Config) -> bool {
    use std::os::windows::process::CommandExt;
    use std::process::Stdio;
    use windows_sys::Win32::System::Console::GetConsoleWindow;
    use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;

    if !config.behavior.launch_without_terminal || std::env::var_os(DETACHED_LAUNCH_ENV).is_some() {
        return false;
    }

    let console = unsafe { GetConsoleWindow() };
    if console.is_null() {
        return false;
    }

    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(error) => {
            tracing::warn!(
                "Failed to determine current executable for relaunch: {}",
                error
            );
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
    if let Err(error) = updater::maybe_prompt_for_update(&app_state).await {
        tracing::warn!("Auto update check failed: {}", error);
    }

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

        let poll_heartbeat = Arc::new(parking_lot::Mutex::new(std::time::Instant::now()));
        let mut data_task = tokio::spawn(app::run_data_loop(
            Arc::clone(&app_state),
            Arc::clone(&api),
            Arc::clone(&star_client),
            Arc::clone(&quit_flag),
            Arc::clone(&poll_heartbeat),
        ));

        loop {
            tokio::select! {
                result = &mut data_task => {
                    if let Err(error) = result {
                        tracing::error!("Data loop task failed: {}", error);
                    }
                    break;
                }
                _ = tokio::time::sleep(DATA_LOOP_WATCHDOG_INTERVAL) => {
                    if poll_heartbeat.lock().elapsed() >= DATA_LOOP_STALL_TIMEOUT {
                        tracing::warn!(
                            "Data loop has not started a poll in {} seconds; restarting session bootstrap",
                            DATA_LOOP_STALL_TIMEOUT.as_secs()
                        );
                        data_task.abort();
                        let _ = data_task.await;
                        break;
                    }
                }
            }
        }

        // Deregister from star backend when the session ends
        if config.star.enabled {
            if let Err(e) = star_client.deregister().await {
                tracing::warn!("Star deregistration failed: {}", e);
            }
        }

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
    settings_requested: Arc<AtomicBool>,
) {
    use egui_overlay::EguiOverlay;

    struct StarOverlay {
        app_state: Arc<RwLock<AppState>>,
        quit_flag: Arc<AtomicBool>,
        key_held: Arc<AtomicBool>,
        tray: Option<tray::SystemTray>,
        settings_requested: Arc<AtomicBool>,
        initialized: bool,
        shown: bool,
        topmost_active: bool,
        settings_open: bool,
        settings_window_active: bool,
        settings_state: settings::SettingsState,
    }

    impl EguiOverlay for StarOverlay {
        fn run(
            &mut self,
            egui_context: &egui::Context,
            default_gfx_backend: &mut egui_overlay::egui_render_three_d::ThreeDBackend,
            glfw_backend: &mut egui_overlay::egui_window_glfw_passthrough::GlfwBackend,
        ) -> Option<(egui::PlatformOutput, std::time::Duration)> {
            if self.settings_window_active && glfw_backend.window.should_close() {
                glfw_backend.window.set_should_close(false);
                self.settings_open = false;
                restore_overlay_window(glfw_backend);
                self.settings_window_active = false;
                self.settings_state.clear();
            }

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

            glfw_backend.set_passthrough(!(self.settings_open || self.settings_window_active));

            let mut input = glfw_backend.take_raw_input();
            if self.settings_open || self.settings_window_active {
                for event in &mut input.events {
                    if let egui::Event::MouseWheel { delta, .. } = event {
                        *delta *= SETTINGS_SCROLL_MULTIPLIER;
                    }
                }
            }
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
                tray.poll_events();
            }

            let settings_open_requested = self.settings_requested.swap(false, Ordering::Relaxed);
            if settings_open_requested {
                self.settings_open = true;
                if self.settings_window_active {
                    glfw_backend.window.show();
                    glfw_backend.window.focus();
                } else {
                    let config = {
                        let state = self.app_state.blocking_read();
                        state.config.clone()
                    };
                    self.settings_state.begin(config);
                }
            }

            if self.settings_open {
                if !self.settings_window_active {
                    set_overlay_topmost(glfw_backend, false);
                    self.topmost_active = false;
                    activate_settings_window(glfw_backend);
                    self.settings_window_active = true;
                    self.shown = true;
                }

                settings::render(
                    egui_context,
                    &self.app_state,
                    &self.quit_flag,
                    &mut self.settings_state,
                    &mut self.settings_open,
                );

                if !self.settings_open {
                    restore_overlay_window(glfw_backend);
                    self.settings_window_active = false;
                    self.shown = false;
                    self.settings_state.clear();
                }

                egui_context.request_repaint_after(std::time::Duration::from_millis(16));
                return;
            }

            if self.settings_window_active {
                restore_overlay_window(glfw_backend);
                self.settings_window_active = false;
                self.shown = false;
                self.settings_state.clear();
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

            if should_be_topmost && !self.shown {
                glfw_backend.window.show();
                self.shown = true;
            } else if !should_be_topmost && self.shown {
                glfw_backend.window.hide();
                self.shown = false;
            }

            egui_context.request_repaint_after(std::time::Duration::from_millis(16));
        }
    }

    start_overlay(StarOverlay {
        app_state,
        quit_flag,
        key_held,
        tray,
        settings_requested,
        initialized: false,
        shown: false,
        topmost_active: false,
        settings_open: false,
        settings_window_active: false,
        settings_state: settings::SettingsState::default(),
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

    set_window_icon(&mut glfw_backend);
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

fn activate_settings_window(
    glfw_backend: &mut egui_overlay::egui_window_glfw_passthrough::GlfwBackend,
) {
    glfw_backend.set_title("Star Client Settings".to_string());
    glfw_backend.window.set_should_close(false);
    glfw_backend.window.set_decorated(true);
    glfw_backend.window.set_resizable(true);
    glfw_backend.window.set_focus_on_show(true);
    glfw_backend.window.set_floating(false);
    glfw_backend.window.set_size_limits(
        Some(settings::MIN_WINDOW_SIZE[0]),
        Some(settings::MIN_WINDOW_SIZE[1]),
        None,
        None,
    );
    glfw_backend.set_passthrough(false);
    glfw_backend.set_window_size(settings::INITIAL_WINDOW_SIZE);
    center_window(glfw_backend);

    #[cfg(target_os = "windows")]
    set_settings_window_styles(glfw_backend, true);

    glfw_backend.window.show();
    glfw_backend.window.focus();
}

fn center_window(glfw_backend: &mut egui_overlay::egui_window_glfw_passthrough::GlfwBackend) {
    let (window_w, window_h) = glfw_backend.window.get_size();
    let Some((work_x, work_y, work_w, work_h)) = glfw_backend
        .glfw
        .with_primary_monitor(|_, monitor| monitor.map(|monitor| monitor.get_workarea()))
    else {
        return;
    };

    let x = work_x + ((work_w - window_w).max(0) / 2);
    let y = work_y + ((work_h - window_h).max(0) / 2);
    glfw_backend.window.set_pos(x, y);
}

fn restore_overlay_window(
    glfw_backend: &mut egui_overlay::egui_window_glfw_passthrough::GlfwBackend,
) {
    glfw_backend.set_title("Star Client".to_string());
    glfw_backend.window.set_should_close(false);
    glfw_backend.window.set_size_limits(None, None, None, None);
    glfw_backend.window.set_resizable(false);
    glfw_backend.window.set_decorated(false);
    glfw_backend.window.set_focus_on_show(false);

    #[cfg(target_os = "windows")]
    set_settings_window_styles(glfw_backend, false);

    init_window(glfw_backend);
    glfw_backend.window.hide();
}

fn set_window_icon(glfw_backend: &mut egui_overlay::egui_window_glfw_passthrough::GlfwBackend) {
    use egui_overlay::egui_window_glfw_passthrough::glfw::PixelImage;

    let icons = [16, 32, 48, 64, 128]
        .into_iter()
        .filter_map(|size| {
            let (pixels, width, height) = assets::window_icon_rgba(size).ok()?;
            Some(PixelImage {
                width,
                height,
                pixels,
            })
        })
        .collect::<Vec<_>>();

    if !icons.is_empty() {
        glfw_backend.window.set_icon_from_pixels(icons);
    }
}

#[cfg(target_os = "windows")]
fn set_settings_window_styles(
    glfw_backend: &mut egui_overlay::egui_window_glfw_passthrough::GlfwBackend,
    settings_mode: bool,
) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE, SWP_FRAMECHANGED,
        SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT,
    };

    let hwnd = glfw_backend.window.get_win32_window();
    if hwnd.is_null() {
        return;
    }

    unsafe {
        let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let next_style = if settings_mode {
            (ex_style | WS_EX_APPWINDOW as isize)
                & !(WS_EX_TOOLWINDOW as isize)
                & !(WS_EX_TRANSPARENT as isize)
        } else {
            (ex_style | WS_EX_TOOLWINDOW as isize | WS_EX_TRANSPARENT as isize)
                & !(WS_EX_APPWINDOW as isize)
        };
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, next_style);
        SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED,
        );
    }
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
