//! ScreenBlocker — Tauri v2 app entry: tray, windows, and the cycle loop.

mod commands;
mod meeting;
mod messages;
mod store;
mod timer;
mod tray_icon;

use commands::{AppState, CurrentBreak};
use store::{EventType, Store};
use tauri::{
    menu::{CheckMenuItemBuilder, MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent,
};
use tauri_plugin_notification::NotificationExt;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(|app| {
            let store = Store::new();
            let settings = store.load_settings();
            // Persist defaults on first run so the file exists.
            let _ = store.save_settings(&settings);
            let start_on_login = settings.start_on_login;
            app.manage(AppState::new(store, settings));

            commands::apply_autostart(app.handle(), start_on_login);
            build_tray(app.handle())?;

            // Dashboard window hides instead of quitting on close.
            if let Some(main) = app.get_webview_window("main") {
                let h = app.handle().clone();
                main.on_window_event(move |ev| {
                    if let WindowEvent::CloseRequested { api, .. } = ev {
                        api.prevent_close();
                        if let Some(w) = h.get_webview_window("main") {
                            let _ = w.hide();
                        }
                    }
                });
            }

            let handle = app.handle().clone();
            std::thread::spawn(move || tick_loop(handle));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_dashboard_data,
            commands::get_settings,
            commands::set_settings,
            commands::get_engine_state,
            commands::toggle_pause,
            commands::set_meeting_override,
            commands::get_current_break,
            commands::skip_break,
            commands::get_meeting_active,
        ])
        .run(tauri::generate_context!())
        .expect("error while running ScreenBlocker");
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItemBuilder::with_id("open", "Open dashboard").build(app)?;
    let meeting = CheckMenuItemBuilder::with_id("meeting", "Force meeting mode").build(app)?;
    let pause = MenuItemBuilder::with_id("pause", "Pause / Resume").build(app)?;
    let quit = MenuItemBuilder::with_id("quit", "Quit").build(app)?;
    let menu = MenuBuilder::new(app)
        .items(&[&open, &meeting])
        .separator()
        .items(&[&pause, &quit])
        .build()?;

    let initial = {
        let state = app.state::<AppState>();
        let s = state.engine.lock().unwrap().state();
        s.remaining_secs
    };

    TrayIconBuilder::with_id("main")
        .icon(tray_icon::time_icon(initial))
        .menu(&menu)
        .tooltip("ScreenBlocker")
        .on_menu_event(|app, event| handle_menu(app, event.id().as_ref()))
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button, button_state, .. } = event {
                if button == MouseButton::Left && button_state == MouseButtonState::Up {
                    show_dashboard(tray.app_handle());
                }
            }
        })
        .build(app)?;
    Ok(())
}

fn handle_menu(app: &AppHandle, id: &str) {
    match id {
        "open" => show_dashboard(app),
        "pause" => {
            let state = app.state::<AppState>();
            state.engine.lock().unwrap().toggle_pause();
        }
        "meeting" => {
            let state = app.state::<AppState>();
            let mut s = state.settings.lock().unwrap();
            s.manual_meeting_override = match s.manual_meeting_override {
                Some(true) => None,
                _ => Some(true),
            };
            let _ = state.store.save_settings(&s);
        }
        "quit" => app.exit(0),
        _ => {}
    }
}

fn show_dashboard(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

fn notify(app: &AppHandle, title: &str, body: &str) {
    let _ = app.notification().builder().title(title).body(body).show();
}

fn open_overlay(app: &AppHandle) {
    if app.get_webview_window("overlay").is_some() {
        return;
    }
    let _ = WebviewWindowBuilder::new(app, "overlay", WebviewUrl::App("overlay.html".into()))
        .title("Time to move")
        .fullscreen(true)
        .always_on_top(true)
        .decorations(false)
        .skip_taskbar(true)
        .focused(true)
        .build();
}

fn close_overlay(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("overlay") {
        let _ = w.close();
    }
}

fn tick_loop(app: AppHandle) {
    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));

        let tick = {
            let state = app.state::<AppState>();
            let t = state.engine.lock().unwrap().tick();
            t
        };
        match tick {
            timer::Tick::WorkEnded => on_work_ended(&app),
            timer::Tick::BreakEnded => on_break_ended(&app),
            timer::Tick::None => {}
        }

        let remaining = {
            let state = app.state::<AppState>();
            let s = state.engine.lock().unwrap().state();
            let _ = app.emit("engine-state", s);
            s.remaining_secs
        };
        if let Some(tray) = app.tray_by_id("main") {
            let _ = tray.set_icon(Some(tray_icon::time_icon(remaining)));
        }
    }
}

fn on_work_ended(app: &AppHandle) {
    let state = app.state::<AppState>();
    let settings = state.settings.lock().unwrap().clone();
    let is_meeting =
        meeting::meeting_state(settings.manual_meeting_override, settings.auto_detect_meetings);

    if is_meeting {
        let body = messages::random_meeting();
        notify(app, "You're on a call — stand up anyway 🧍", body);
        let _ = state.store.append_event(EventType::MeetingNotified);
        state.engine.lock().unwrap().restart_work();
    } else {
        let (title, body) = messages::random_stretch();
        let successful_today = state.store.dashboard_data(1).today_successful;
        let break_secs = state.engine.lock().unwrap().state().break_secs;
        *state.current_break.lock().unwrap() = Some(CurrentBreak {
            title: title.to_string(),
            body: body.to_string(),
            break_secs,
            successful_today,
        });
        state.engine.lock().unwrap().start_break();
        open_overlay(app);
    }
}

fn on_break_ended(app: &AppHandle) {
    let state = app.state::<AppState>();
    let _ = state.store.append_event(EventType::Successful);
    *state.current_break.lock().unwrap() = None;
    close_overlay(app);
    notify(
        app,
        "Nice — break complete ✓",
        "Back to focus. Your next break is on the clock.",
    );
}
