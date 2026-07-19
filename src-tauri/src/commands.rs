//! Tauri commands invoked from the frontend, plus shared app state.

use crate::meeting;
use crate::store::{DashboardData, Settings, Store};
use crate::timer::{Engine, EngineState};
use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_autostart::ManagerExt;

/// Info handed to the overlay window when a break starts.
#[derive(Debug, Clone, Serialize)]
pub struct CurrentBreak {
    pub title: String,
    pub body: String,
    pub break_secs: i64,
    pub successful_today: u32,
}

pub struct AppState {
    pub store: Store,
    pub engine: Mutex<Engine>,
    pub settings: Mutex<Settings>,
    pub current_break: Mutex<Option<CurrentBreak>>,
}

impl AppState {
    pub fn new(store: Store, settings: Settings) -> Self {
        let engine = Engine::new(
            settings.work_minutes as i64 * 60,
            settings.break_minutes as i64 * 60,
        );
        AppState {
            store,
            engine: Mutex::new(engine),
            settings: Mutex::new(settings),
            current_break: Mutex::new(None),
        }
    }
}

#[tauri::command]
pub fn get_dashboard_data(state: State<AppState>) -> DashboardData {
    state.store.dashboard_data(14)
}

#[tauri::command]
pub fn get_settings(state: State<AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
pub fn set_settings(app: AppHandle, state: State<AppState>, new: Settings) -> Settings {
    {
        let mut engine = state.engine.lock().unwrap();
        engine.set_durations(new.work_minutes as i64 * 60, new.break_minutes as i64 * 60);
    }
    apply_autostart(&app, new.start_on_login);
    *state.settings.lock().unwrap() = new.clone();
    let _ = state.store.save_settings(&new);
    new
}

#[tauri::command]
pub fn get_engine_state(state: State<AppState>) -> EngineState {
    state.engine.lock().unwrap().state()
}

#[tauri::command]
pub fn toggle_pause(state: State<AppState>) -> EngineState {
    let mut engine = state.engine.lock().unwrap();
    engine.toggle_pause();
    engine.state()
}

#[tauri::command]
pub fn set_meeting_override(state: State<AppState>, value: Option<bool>) -> Settings {
    let mut settings = state.settings.lock().unwrap();
    settings.manual_meeting_override = value;
    let _ = state.store.save_settings(&settings);
    settings.clone()
}

#[tauri::command]
pub fn get_current_break(state: State<AppState>) -> Option<CurrentBreak> {
    state.current_break.lock().unwrap().clone()
}

/// Skip the in-progress break. Logs it as skipped and closes the overlay.
#[tauri::command]
pub fn skip_break(app: AppHandle, state: State<AppState>) {
    let skipped = state.engine.lock().unwrap().skip_break();
    if skipped {
        let _ = state.store.append_event(crate::store::EventType::Skipped);
    }
    *state.current_break.lock().unwrap() = None;
    if let Some(w) = app.get_webview_window("overlay") {
        let _ = w.close();
    }
}

/// Effective meeting state right now (for the settings UI indicator).
#[tauri::command]
pub fn get_meeting_active(state: State<AppState>) -> bool {
    let s = state.settings.lock().unwrap().clone();
    meeting::meeting_state(s.manual_meeting_override, s.auto_detect_meetings)
}

pub fn apply_autostart(app: &AppHandle, enable: bool) {
    let manager = app.autolaunch();
    let _ = if enable {
        manager.enable()
    } else {
        manager.disable()
    };
}
