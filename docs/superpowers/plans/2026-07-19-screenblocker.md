# ScreenBlocker Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A Tauri v2 desktop app for KDE Plasma / Wayland that runs a focus→break cycle, blocks the screen (or notifies during meetings) to enforce stand-up breaks, shows a live countdown in the panel tray, and tracks break history in JSON with a dashboard.

**Architecture:** Rust backend owns a timer state machine, PipeWire meeting detection, a JSON store, and a dynamic tray icon that renders the remaining minutes. Two WebView windows (dashboard + fullscreen overlay) built from plain HTML/CSS/JS derived from the approved mockup. Frontend talks to Rust via Tauri commands and events.

**Tech Stack:** Tauri v2, Rust (stable), tokio, serde/serde_json, directories, resvg/usvg/tiny-skia (tray text rendering), plain HTML/CSS/JS frontend, PipeWire (`pw-dump`).

## Global Constraints

- Platform: Linux / KDE Plasma / Wayland only. Single user.
- Tauri **v2** (not v1). WebKitGTK 4.1.
- Storage: `~/.local/share/screenblocker/history.json`; settings `~/.config/screenblocker/settings.json`.
- Event types logged: `successful | skipped | meeting_notified`.
- Defaults: work 25 min, break 5 min, auto-detect meetings on, start-on-login on.
- Panel countdown = minutes remaining; `mm:ss` only in the final minute.
- Meeting break = notification only (no block); restart cycle, do not re-nag.
- Manual meeting override wins over auto-detect when set.
- Not a security kiosk — overlay is fullscreen always-on-top, escapable (accepted).
- Frontend derived from approved `mockup.html`; theme-aware, dark default; validated chart palette (success `#2f9e6f`/`#3aa981`, skip `#cf5340`/`#d9614e`).

---

## Task 0: Environment setup (one-time)

**Deliverable:** Rust toolchain + Tauri Linux deps installed; `cargo` and `tauri` CLI available.

- [ ] Install system build deps (needs sudo — run by user):
  `sudo pacman -S --needed webkit2gtk-4.1 base-devel curl wget file openssl librsvg libappindicator-gtk3 patchelf`
- [ ] Install rustup + stable toolchain (no sudo): `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y`
- [ ] Install Tauri CLI: `npm install -g @tauri-apps/cli@latest` (or use `npx tauri`).
- [ ] Verify: `cargo --version`, `npx tauri --version`.

## Task 1: Scaffold Tauri v2 project

**Files:** `package.json`, `src-tauri/` (Cargo.toml, tauri.conf.json, build.rs, src/main.rs), `src/index.html`.

- [ ] Scaffold with `npm create tauri-app@latest` (vanilla, TypeScript off) or manual: create `src-tauri` crate + `src/` frontend dir, wire `tauri.conf.json` `frontendDist` to `../src`, no dev server (static).
- [ ] Configure two windows in `tauri.conf.json`: `main` (dashboard, `visible:false`, closable→hide) and no static overlay (created at runtime).
- [ ] Add Cargo deps: `tauri` (v2, features tray-icon, image-png), `tokio`, `serde`, `serde_json`, `directories`, `chrono`, `resvg`, `usvg`, `tiny-skia`.
- [ ] Build once: `cargo build` under `src-tauri` (compiles empty app).
- [ ] Commit.

## Task 2: Store module (`src-tauri/src/store.rs`)

**Interfaces produced:** `Settings` struct (work_minutes,break_minutes,auto_detect_meetings,manual_meeting_override:Option<bool>,start_on_login); `load_settings()`, `save_settings(&Settings)`; `Event{ts:String,type}`; `append_event(EventType)`; `dashboard_data(days)->DashboardData` (per-day successful/skipped counts, today totals, streak, all-time total).

- [ ] Test: append_event writes to temp history file and dashboard_data aggregates by day (seed events across days, assert counts/streak).
- [ ] Implement with serde_json + directories; create dirs if missing; atomic write (temp+rename).
- [ ] Run tests → pass. Commit.

## Task 3: Timer/cycle engine (`src-tauri/src/timer.rs`)

**Interfaces produced:** `Phase{Work,Break,Paused}`; `EngineState{phase,remaining_secs,work_secs,break_secs}`; `Engine` with `tick()`, `start_break()`, `complete_break()`, `skip_break()`, `pause()/resume()`, `on_phase_change` callback. Engine is pure logic (no Tauri), advanced by `tick()` once/sec.

- [ ] Test: from Work with remaining=1, tick() twice → phase transitions to Break-request; break countdown to 0 → complete; skip mid-break → skipped; pause halts countdown.
- [ ] Implement state machine. Emit transition intents via callback (start_break/break_ended-successful) so the Tauri layer performs side effects (windows, logging).
- [ ] Run tests → pass. Commit.

## Task 4: Meeting detector (`src-tauri/src/meeting.rs`)

**Interfaces produced:** `is_capture_active()->bool` (parses `pw-dump` JSON for nodes with `media.class` == `Stream/Input/Audio` or `Stream/Input/Video`); `meeting_state(settings)->bool` applying manual override.

- [ ] Test: `parse_pw_dump(json)->bool` given fixture JSON with an input-audio stream → true; with only monitor/output → false. (Pure parse fn is the unit under test.)
- [ ] Implement: run `pw-dump`, parse with serde_json, detect active capture streams; `meeting_state` returns override if `Some`, else auto-detect.
- [ ] Run tests → pass. Commit.

## Task 5: Tray icon renderer (`src-tauri/src/tray_icon.rs`)

**Interfaces produced:** `render_time_icon(text:&str)->tauri::image::Image` — rasterizes an SVG with the given text (e.g. "24" or "0:45") to a 32×32 (and 64×64) RGBA image via usvg+resvg+tiny-skia.

- [ ] Test: `render_rgba("24")` returns non-empty RGBA buffer of expected dimensions (smoke test — no golden image).
- [ ] Implement SVG string with centered text, rasterize, return RGBA bytes → `Image::new`.
- [ ] Run test → pass. Commit.

## Task 6: Messages bank (`src-tauri/src/messages.rs`)

**Interfaces produced:** `random_stretch()->(title,body)`; `random_meeting()->String`. Static lists (from mockup), simple index pick.

- [ ] Test: functions return non-empty strings; pick varies across calls (seeded/indexed).
- [ ] Implement using a rotating index (no rand dep needed) or `fastrand`.
- [ ] Run test → pass. Commit.

## Task 7: Tauri commands + app wiring (`src-tauri/src/lib.rs`, `commands.rs`, `main.rs`)

**Interfaces produced (commands):** `get_dashboard_data`, `get_settings`, `set_settings`, `get_engine_state`, `toggle_pause`, `set_meeting_override(Option<bool>)`, `skip_break`, `get_current_break` (title/body/break_secs for overlay).

- [ ] Build tray icon (TrayIconBuilder) with menu: Open dashboard, Force meeting mode (checkable), Pause/Resume, Quit. Update icon each tick via `render_time_icon`.
- [ ] Spawn 1s tick task: advance Engine; on Work→Break intent, check `meeting_state`: if meeting → `append_event(meeting_notified)` + notification + restart Work; else create fullscreen always-on-top overlay window (`WebviewWindowBuilder`, fullscreen, always_on_top, decorations:false, skip_taskbar) loading `overlay.html`. On break complete → `append_event(successful)` + close overlay. `skip_break` command → `append_event(skipped)` + close overlay + resume Work.
- [ ] Dashboard window: show on tray "Open"; intercept close → hide.
- [ ] Register commands, run app. Manual smoke: `npx tauri dev`.
- [ ] Commit.

## Task 8: Frontend — dashboard (`src/index.html`, `dashboard.js`, `styles.css`)

**Consumes:** `get_dashboard_data`, `get_settings`, `set_settings`, `get_engine_state`.

- [ ] Port dashboard + settings markup/CSS from `mockup.html`; replace sample data with `invoke('get_dashboard_data')`; wire settings steppers/toggles to `set_settings`; poll `get_engine_state` for the "next break in Xm" status line.
- [ ] Keep the validated SVG chart (real data), hover tooltips, theme handling.
- [ ] Manual verify in `tauri dev`. Commit.

## Task 9: Frontend — overlay (`src/overlay.html`, `overlay.js`)

**Consumes:** `get_current_break`, `skip_break`; listens for close.

- [ ] Port overlay markup/CSS from mockup; on load call `get_current_break` → set title/body + break_secs; run local countdown ring; Skip button → `invoke('skip_break')`.
- [ ] Manual verify: trigger a break (temporarily set work=1min). Commit.

## Task 10: Autostart + packaging

- [ ] Add `tauri-plugin-autostart` (or write a `.desktop` file to `~/.config/autostart/`) wired to the start-on-login setting.
- [ ] `npx tauri build` → produce binary/bundle; verify it launches, tray countdown ticks, a real break blocks the screen, meeting mode notifies.
- [ ] Write `README.md` (setup, run, data location). Commit.

## Self-Review

- **Spec coverage:** cycle (T3,T7), meeting detect+override (T4,T7), tray countdown (T5,T7), overlay block (T7,T9), notification (T7), dashboard+chart (T8), JSON store+model (T2), settings (T2,T8), autostart (T10), out-of-scope respected. ✓
- **Placeholders:** none — each task has concrete files/interfaces/tests.
- **Type consistency:** `Settings`, `EngineState`, `EventType`, command names reused consistently across tasks. ✓
- Deferred detail from spec (streak threshold) fixed in T2: streak = consecutive days with ≥1 successful break (simpler than mockup's "≥6"; tunable constant `STREAK_MIN_SUCCESS`).
