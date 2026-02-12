#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

mod agent;
mod api;
mod bash;
mod browser;
mod computer;
mod gemini;
mod panels;
mod permissions;
mod storage;
mod voice;

use agent::{Agent, AgentMode, HistoryMessage};
use std::sync::Arc;
use tokio::sync::Mutex;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Emitter, Manager, PhysicalPosition, State,
};
use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut, ShortcutState};

#[cfg(target_os = "macos")]
use tauri_nspanel::{
    tauri_panel, CollectionBehavior, ManagerExt, PanelLevel, StyleMask, WebviewWindowExt,
};

#[cfg(target_os = "macos")]
tauri_panel! {
    panel!(OMARAIPanel {
        config: {
            can_become_key_window: true,
            is_floating_panel: true
        }
    })
}

struct AppState {
    agent: Arc<Mutex<Agent>>,
    running: Arc<std::sync::atomic::AtomicBool>,
}

// cached screen info for fast window positioning
#[cfg(target_os = "macos")]
struct ScreenInfo {
    width: f64,
    height: f64,
    menubar_height: f64,
    scale: f64,
}

#[cfg(target_os = "macos")]
static SCREEN_INFO: std::sync::OnceLock<ScreenInfo> = std::sync::OnceLock::new();

// re-export panel handles from shared module
#[cfg(target_os = "macos")]
use panels::{MAIN_PANEL, VOICE_PANEL, BORDER_PANEL};

#[cfg(target_os = "macos")]
fn get_screen_info() -> &'static ScreenInfo {
    SCREEN_INFO.get_or_init(|| {
        use objc2_app_kit::NSScreen;
        use objc2_foundation::MainThreadMarker;

        if let Some(mtm) = MainThreadMarker::new() {
            if let Some(screen) = NSScreen::mainScreen(mtm) {
                let frame = screen.frame();
                let visible = screen.visibleFrame();
                let menubar_height = frame.size.height - visible.size.height - visible.origin.y;
                let scale = screen.backingScaleFactor();
                return ScreenInfo {
                    width: frame.size.width,
                    height: frame.size.height,
                    menubar_height,
                    scale,
                };
            }
        }
        // fallback for retina mac
        ScreenInfo { width: 1440.0, height: 900.0, menubar_height: 25.0, scale: 2.0 }
    })
}

#[cfg(target_os = "macos")]
fn position_window_top_right(window: &tauri::WebviewWindow, width: f64, _height: f64) {
    let info = get_screen_info();
    let padding = 10.0;
    let x = (info.width - width - padding) * info.scale;
    let y = (info.menubar_height + padding) * info.scale;
    let _ = window.set_position(PhysicalPosition::new(x as i32, y as i32));
}

#[cfg(target_os = "macos")]
fn position_window_center(window: &tauri::WebviewWindow, width: f64, height: f64) {
    let info = get_screen_info();
    let x = ((info.width - width) / 2.0) * info.scale;
    let y = ((info.height - height) / 2.0) * info.scale;
    let _ = window.set_position(PhysicalPosition::new(x as i32, y as i32));
}

#[tauri::command]
async fn set_api_key(api_key: String, state: State<'_, AppState>) -> Result<(), String> {
    let mut agent = state.agent.lock().await;
    agent.set_api_key(api_key);
    Ok(())
}

#[tauri::command]
async fn check_api_key(state: State<'_, AppState>) -> Result<bool, String> {
    let agent = state.agent.lock().await;
    Ok(agent.has_api_key())
}

#[tauri::command(rename_all = "camelCase")]
async fn run_agent(
    instructions: String,
    model: String,
    mode: AgentMode,
    voice_mode: Option<bool>,
    history: Vec<HistoryMessage>,
    context_screenshot: Option<String>,
    conversation_id: Option<String>,
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let voice = voice_mode.unwrap_or(false);
    println!("[omar-ai] run_agent called with: {} (model: {}, mode: {:?}, voice: {}, history: {} msgs, screenshot: {}, conv: {:?})",
        instructions, model, mode, voice, history.len(), context_screenshot.is_some(), conversation_id);

    let agent = state.agent.clone();

    {
        let agent_guard = agent.lock().await;
        if agent_guard.is_running() {
            return Err("Agent is already running".to_string());
        }
        if !agent_guard.has_api_key() {
            return Err("No API key set. Please add API_KEY in settings".to_string());
        }
    }

    tokio::spawn(async move {
        let agent_guard = agent.lock().await;
        match agent_guard.run(instructions, model, mode, voice, history, context_screenshot, conversation_id, app_handle).await {
            Ok(_) => println!("[omar-ai] Agent finished"),
            Err(e) => println!("[omar-ai] Agent error: {:?}", e),
        }
    });

    Ok(())
}

#[tauri::command]
fn stop_agent(state: State<'_, AppState>) -> Result<(), String> {
    state.running.store(false, std::sync::atomic::Ordering::SeqCst);
    println!("[omar-ai] Stop requested");
    Ok(())
}

#[tauri::command]
fn is_agent_running(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(state.running.load(std::sync::atomic::Ordering::SeqCst))
}

#[tauri::command]
fn debug_log(message: String) {
    println!("[frontend] {}", message);
}

// unified window state command - frontend tells backend what size/position it needs
#[tauri::command]
fn set_window_state(app_handle: tauri::AppHandle, width: f64, height: f64, centered: bool) -> Result<(), String> {
    println!("[window] set_window_state: {}x{}, centered={}", width, height, centered);
    #[cfg(target_os = "macos")]
    {
        if let Some(window) = app_handle.get_webview_window("main") {
            let _ = window.set_size(tauri::LogicalSize::new(width, height));
            if centered {
                position_window_center(&window, width, height);
            } else {
                position_window_top_right(&window, width, height);
            }
            if let Some(panel) = MAIN_PANEL.get() {
                panel.show();
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        if let Some(window) = app_handle.get_webview_window("main") {
            let _ = window.set_size(tauri::LogicalSize::new(width, height));
            let _ = window.show();
        }
    }
    Ok(())
}

// voice window controls
#[tauri::command]
fn show_voice_window(app_handle: tauri::AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        if let Some(window) = app_handle.get_webview_window("voice") {
            position_window_center(&window, 300.0, 300.0);
        }
        if let Some(panel) = VOICE_PANEL.get() {
            panel.show();
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        if let Some(window) = app_handle.get_webview_window("voice") {
            let _ = window.center();
            let _ = window.show();
        }
    }
    Ok(())
}

#[tauri::command]
fn hide_voice_window(_app_handle: tauri::AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    if let Some(panel) = VOICE_PANEL.get() {
        panel.hide();
    }
    #[cfg(not(target_os = "macos"))]
    if let Some(window) = _app_handle.get_webview_window("voice") {
        let _ = window.hide();
    }
    Ok(())
}

#[tauri::command]
fn hide_main_window(_app_handle: tauri::AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    if let Some(panel) = MAIN_PANEL.get() {
        panel.hide();
    }
    #[cfg(not(target_os = "macos"))]
    if let Some(window) = _app_handle.get_webview_window("main") {
        let _ = window.hide();
    }
    Ok(())
}

// show main window in voice response mode and emit event
#[tauri::command]
fn show_main_voice_response(app_handle: tauri::AppHandle, text: String, screenshot: Option<String>, mode: String) -> Result<(), String> {
    // emit event to main window so it can switch to voice response mode
    let _ = app_handle.emit("voice:response", serde_json::json!({
        "text": text,
        "screenshot": screenshot,
        "mode": mode,
    }));

    // show main panel (frontend will handle sizing via set_window_state)
    #[cfg(target_os = "macos")]
    if let Some(panel) = MAIN_PANEL.get() {
        panel.show();
    }
    #[cfg(not(target_os = "macos"))]
    if let Some(window) = app_handle.get_webview_window("main") {
        let _ = window.show();
    }

    Ok(())
}

// set main panel click-through (ignores mouse events)
#[tauri::command]
fn set_main_click_through(ignore: bool) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    if let Some(panel) = MAIN_PANEL.get() {
        panel.set_ignores_mouse_events(ignore);
    }
    Ok(())
}

#[tauri::command]
fn show_border_overlay() {
    #[cfg(target_os = "macos")]
    if let Some(panel) = BORDER_PANEL.get() {
        panel.show();
    }
}

#[tauri::command]
fn hide_border_overlay() {
    #[cfg(target_os = "macos")]
    if let Some(panel) = BORDER_PANEL.get() {
        panel.hide();
    }
}

// take screenshot excluding our app windows - uses shared panels module
#[tauri::command]
fn take_screenshot_excluding_app() -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        panels::take_screenshot_excluding_app()
    }

    #[cfg(not(target_os = "macos"))]
    {
        let control = computer::ComputerControl::new().map_err(|e| e.to_string())?;
        control.take_screenshot().map_err(|e| e.to_string())
    }
}

// trigger screen flash effect - plays sound as feedback
#[cfg(target_os = "macos")]
fn trigger_screen_flash() {
    std::process::Command::new("afplay")
        .arg("/System/Library/Components/CoreAudio.component/Contents/SharedSupport/SystemSounds/system/Grab.aif")
        .spawn()
        .ok();
}

// hotkey triggered - capture screenshot and return base64
#[tauri::command]
fn capture_screen_for_help() -> Result<String, String> {
    let control = computer::ComputerControl::new().map_err(|e| e.to_string())?;
    let screenshot = control.take_screenshot().map_err(|e| e.to_string())?;
    
    #[cfg(target_os = "macos")]
    trigger_screen_flash();

    Ok(screenshot)
}

#[tauri::command]
async fn start_ptt(mode: String, app_handle: tauri::AppHandle) -> Result<(), String> {
    voice::start_listening(mode, app_handle).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn stop_ptt() -> Result<(), String> {
    voice::stop_listening().await.map_err(|e| e.to_string())
}

#[tauri::command]
fn check_permissions() -> Result<serde_json::Value, String> {
    Ok(permissions::check_all())
}

#[tauri::command]
fn request_permission(permission: String) -> Result<(), String> {
    permissions::request(&permission)
}

#[tauri::command]
fn open_permission_settings(permission: String) -> Result<(), String> {
    permissions::open_settings(&permission)
}

#[tauri::command]
async fn open_browser_profile() -> Result<(), String> {
    browser::open_profile_dir().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn open_browser_profile_url(url: String) -> Result<(), String> {
    browser::open_profile_url(&url).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn reset_browser_profile() -> Result<(), String> {
    browser::reset_profile().await.map_err(|e| e.to_string())
}

#[tauri::command]
fn get_browser_profile_status() -> Result<serde_json::Value, String> {
    Ok(browser::get_profile_status())
}

#[tauri::command]
async fn clear_domain_cookies(domain: String) -> Result<(), String> {
    browser::clear_domain_cookies(&domain).await.map_err(|e| e.to_string())
}

#[tauri::command]
fn save_api_key(service: String, key: String) -> Result<(), String> {
    storage::save_api_key(&service, &key)
}

#[tauri::command]
fn get_api_key_status() -> Result<serde_json::Value, String> {
    Ok(storage::get_api_key_status())
}

#[tauri::command]
fn save_voice_settings(voice_id: String) -> Result<(), String> {
    storage::save_voice_settings(&voice_id)
}

#[tauri::command]
fn get_voice_settings() -> Result<serde_json::Value, String> {
    Ok(storage::get_voice_settings())
}

#[tauri::command]
fn list_conversations(limit: usize, offset: usize) -> Result<Vec<storage::ConversationMeta>, String> {
    storage::list_conversations(limit, offset)
}

#[tauri::command]
fn load_conversation(id: String) -> Result<Option<storage::Conversation>, String> {
    storage::load_conversation(&id)
}

#[tauri::command]
fn delete_conversation(id: String) -> Result<(), String> {
    storage::delete_conversation(&id)
}

fn main() {
    if let Err(e) = storage::init_db() {
        eprintln!("Failed to initialize database: {}", e);
    }

    let running = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let agent = Arc::new(Mutex::new(Agent::new(running.clone())));

    // load API keys from DB if they exist
    let keys = storage::get_api_key_status();
    if keys["anthropic"].as_bool().unwrap_or(false) {
        if let Ok(Some(key)) = storage::get_api_key("anthropic") {
            let mut agent_sync = Agent::new(running.clone());
            agent_sync.set_api_key(key);
            // we can't easily update the Arc<Mutex<Agent>> here without a block, 
            // but the first run_agent call will have it. 
            // Better: update the agent in the state below.
        }
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_positioner::init())
        .manage(AppState {
            agent: agent.clone(),
            running: running.clone(),
        })
        .setup(move |app| {
            // register global shortcuts
            let help_shortcut = Shortcut::new(Some(Modifiers::COMMAND | Modifiers::SHIFT), Code::KeyH);
            let spotlight_shortcut = Shortcut::new(Some(Modifiers::COMMAND | Modifiers::SHIFT), Code::Space);
            let stop_shortcut = Shortcut::new(Some(Modifiers::COMMAND | Modifiers::SHIFT), Code::KeyS);
            let ptt_computer_shortcut = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyC);
            let ptt_browser_shortcut = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyB);

            app.handle().plugin(
                tauri_plugin_global_shortcut::Builder::new()
                    .with_handler(move |app, shortcut, event| {
                        if event.state == ShortcutState::Pressed {
                            if shortcut == &help_shortcut {
                                let app_handle = app.clone();
                                tokio::spawn(async move {
                                    if let Ok(screenshot) = capture_screen_for_help() {
                                        let _ = app_handle.emit("hotkey-help", serde_json::json!({ "screenshot": screenshot }));
                                        // show window
                                        #[cfg(target_os = "macos")]
                                        if let Some(panel) = MAIN_PANEL.get() {
                                            panel.show();
                                        }
                                    }
                                });
                            } else if shortcut == &spotlight_shortcut {
                                let _ = app.emit("hotkey-spotlight", ());
                                #[cfg(target_os = "macos")]
                                if let Some(panel) = MAIN_PANEL.get() {
                                    panel.show();
                                }
                            } else if shortcut == &stop_shortcut {
                                let state: State<AppState> = app.state();
                                state.running.store(false, std::sync::atomic::Ordering::SeqCst);
                                println!("[omar-ai] Stop shortcut triggered");
                            } else if shortcut == &ptt_computer_shortcut {
                                let app_handle = app.clone();
                                tokio::spawn(async move {
                                    let _ = voice::start_listening("computer".to_string(), app_handle).await;
                                });
                            } else if shortcut == &ptt_browser_shortcut {
                                let app_handle = app.clone();
                                tokio::spawn(async move {
                                    let _ = voice::start_listening("browser".to_string(), app_handle).await;
                                });
                            }
                        } else if event.state == ShortcutState::Released {
                            if shortcut == &ptt_computer_shortcut || shortcut == &ptt_browser_shortcut {
                                tokio::spawn(async move {
                                    let _ = voice::stop_listening().await;
                                });
                            }
                        }
                    })
                    .build(),
            )?;

            // macos panel setup
            #[cfg(target_os = "macos")]
            {
                let main_window = app.get_webview_window("main").unwrap();
                let voice_window = app.get_webview_window("voice").unwrap();
                let border_window = app.get_webview_window("border").unwrap();

                let main_panel = app.panel_handle("OMARAIPanel");
                let voice_panel = app.panel_handle("OMARAIPanel"); // reuse same config
                let border_panel = app.panel_handle("OMARAIPanel");

                // set as non-activating panels so they don't take focus from other apps
                main_panel.set_collection_behavior(CollectionBehavior::CanJoinAllSpaces | CollectionBehavior::FullScreenAuxiliary);
                main_panel.set_level(PanelLevel::Floating);
                main_panel.set_style_mask(StyleMask::NonActivatingPanel | StyleMask::Titled | StyleMask::FullSizeContentView);
                
                // init global handles
                MAIN_PANEL.set(main_panel).ok();
                VOICE_PANEL.set(voice_panel).ok();
                BORDER_PANEL.set(border_panel).ok();
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            set_api_key,
            check_api_key,
            run_agent,
            stop_agent,
            is_agent_running,
            debug_log,
            set_window_state,
            show_voice_window,
            hide_voice_window,
            hide_main_window,
            show_main_voice_response,
            set_main_click_through,
            show_border_overlay,
            hide_border_overlay,
            take_screenshot_excluding_app,
            capture_screen_for_help,
            start_ptt,
            stop_ptt,
            check_permissions,
            request_permission,
            open_permission_settings,
            open_browser_profile,
            open_browser_profile_url,
            reset_browser_profile,
            get_browser_profile_status,
            clear_domain_cookies,
            save_api_key,
            get_api_key_status,
            save_voice_settings,
            get_voice_settings,
            list_conversations,
            load_conversation,
            delete_conversation,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
