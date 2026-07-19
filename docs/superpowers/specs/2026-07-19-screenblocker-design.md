# ScreenBlocker — Design Spec

**Date:** 2026-07-19
**Status:** Approved (design + UI mockup)
**Target machine:** CachyOS, KDE Plasma, Wayland (single-user personal tool)

## 1. Purpose

A personal "stand up and stretch" enforcer for long, uninterrupted deep-work
sessions. It runs a repeating focus/break cycle, blocks the screen when a break is
due (or nudges gently during meetings), and tracks break history in a local JSON
file so the user can see themselves improving over time.

This is a self-discipline nudge, **not** a security kiosk. On Wayland no app can
truly lock the user out; a fullscreen always-on-top window covering the panel is
the intended and sufficient behavior.

## 2. Tech stack

- **Tauri v2** — Rust backend + system WebView (WebKitGTK) frontend.
- **Frontend:** plain HTML/CSS/JS (no framework), derived from the approved
  `mockup.html`. Self-contained, theme-aware (dark default), inline SVG trend chart.
- **Storage:** single JSON file at `~/.local/share/screenblocker/history.json`.
  Settings stored alongside (or in `~/.config/screenblocker/settings.json`).
- **System integration:**
  - PipeWire (`pw-dump`, fallback `pactl list source-outputs` / `sink-inputs`) to
    detect active mic/camera streams for meeting auto-detect.
  - `notify-send` / Tauri notification API for meeting-mode notices.
  - Dynamic tray icon rendered with the remaining time as the image.
- **One-time setup:** install `rustup` + Rust stable toolchain and Tauri's Linux
  build deps (webkit2gtk-4.1, libappindicator/ayatana, librsvg, etc.).

## 3. Core behavior — the cycle

1. **Work interval** (default 25 min) counts down silently in the background.
   Remaining minutes are rendered directly into the **KDE tray icon** (the panel
   countdown). The number is minutes remaining, switching to `mm:ss` in the final
   minute.
2. When the interval reaches zero, the app checks meeting state:
   - **Not in a meeting** → show the **fullscreen break overlay** (always-on-top,
     covers the panel): random stretch message + break countdown ring
     (default 5 min).
     - Countdown reaches zero → overlay closes automatically → logged
       **`successful`**.
     - User clicks **Skip** → overlay closes early → logged **`skipped`**.
   - **In a meeting** → no blocking. Fire a desktop **notification** with a random
     "you're on a call, stand up anyway" message → logged **`meeting_notified`**.
3. The next work interval starts automatically. (During a meeting-mode break we
   restart the cycle rather than re-nagging.)

### Meeting detection

- **Auto-detect (default on):** a mic or camera capture stream is active via
  PipeWire → treat as meeting.
- **Manual override:** a tray-menu / settings toggle to force meeting mode on or
  off, overriding auto-detect when it guesses wrong (e.g. music playing, or a
  camera-off call). Manual state wins when set.

## 4. UI

Two faces:

### Panel countdown (always running)
- Dynamic tray icon showing minutes remaining (`24`), refreshed as it ticks;
  `mm:ss` in the final minute.
- Tray context menu: Open dashboard, Force meeting mode (toggle), Pause/Resume,
  Quit.

### Dashboard window (on open)
Default view is the **Dashboard**, not settings.
- **Stat tiles:** Successful today, Skipped today, Current streak, All-time total.
- **Trend chart:** grouped bar chart, last **14 days**, successful (green
  `#2f9e6f`/`#3aa981`) vs skipped (coral `#cf5340`/`#d9614e`) per day, per-bar hover
  tooltip. Palette validated colorblind-safe; legend + bar gaps are the secondary
  encoding.
- **Settings tab:** work interval, break length (steppers); auto-detect meetings
  toggle; force-meeting-mode override; start-on-login; data-file location/open
  folder.

Theme-aware (dark default + light), matching the approved `mockup.html`.

## 5. Data model

`history.json` — append-only log of break events:

```json
{
  "events": [
    { "ts": "2026-07-19T14:32:00+05:30", "type": "successful" },
    { "ts": "2026-07-19T15:10:00+05:30", "type": "skipped" },
    { "ts": "2026-07-19T15:48:00+05:30", "type": "meeting_notified" }
  ]
}
```

`type` ∈ `successful | skipped | meeting_notified`. Dashboard aggregates by local
calendar day. Streak = consecutive days meeting a per-day successful threshold
(exact threshold defined in the plan; mockup shows "≥ 6 breaks").

`settings.json`:

```json
{
  "work_minutes": 25,
  "break_minutes": 5,
  "auto_detect_meetings": true,
  "manual_meeting_override": null,
  "start_on_login": true
}
```

## 6. Component boundaries

- **Timer/cycle engine (Rust):** owns the work→break→work state machine, emits
  state to frontend + tray. Testable in isolation.
- **Meeting detector (Rust):** polls PipeWire, exposes `is_meeting_active()`.
  Manual override applied on top. Testable with mocked stream input.
- **Store (Rust):** read/write `history.json` + `settings.json`; append event;
  aggregate for dashboard. Testable against a temp dir.
- **Tray icon renderer (Rust):** turns "minutes remaining" into an icon image.
- **Overlay window + Dashboard window (frontend):** two WebView windows/routes;
  overlay is fullscreen always-on-top, dashboard is a normal window.
- **Message bank:** static lists of stretch + meeting messages, random pick each
  time (may become editable later — out of scope for v1).

## 7. Out of scope for v1 (YAGNI)

- Editable/custom message lists (built-in random lists only).
- Idle/lock detection or pausing on inactivity.
- Multi-monitor per-screen tuning (overlay covers the primary/all as Tauri allows).
- Meeting detection by app-window/tab inspection.
- Cross-platform support (Linux/KDE Wayland only).

## 8. Known limitations (accepted)

- Wayland cannot force a true lockout; overlay is escapable (this is fine).
- Auto-detect can misfire on music/camera-off calls — mitigated by manual override.
- Tray icon shows minutes (not full `mm:ss`) except in the final minute, due to
  icon size.
