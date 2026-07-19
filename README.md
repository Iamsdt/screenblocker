# ScreenBlocker

A personal *stand-up & stretch* enforcer for **KDE Plasma / Wayland**. It runs a
repeating focus→break cycle, blocks your screen when a break is due (or nudges you
gently during meetings), shows a live countdown right in your panel tray, and
tracks your break history so you can watch yourself improve.

Built with **Tauri v2** (Rust backend + WebView UI). Data lives in plain JSON.

## Features

- **Focus → break cycle** — default 25 min work, 5 min break (configurable).
- **Panel countdown** — the remaining time is rendered directly into the KDE tray
  icon (minutes normally; seconds in a coral pill during the final minute).
- **Fullscreen break overlay** — a random stretch prompt + countdown ring. Let it
  run out (logged *successful*) or hit **Skip** (logged *skipped*).
- **Meeting mode** — when your mic/camera is active (auto-detected via PipeWire),
  it shows a desktop notification instead of blocking. A manual override (tray menu
  or settings) is there for when auto-detect guesses wrong.
- **Dashboard** — today's successful vs skipped counts, current streak, all-time
  total, and a 14-day trend chart.
- **Start on login** and everything else from the Settings tab.

## Requirements (one-time setup)

System packages (Arch / CachyOS):

```bash
sudo pacman -S --needed webkit2gtk-4.1 base-devel curl wget file openssl \
  librsvg libappindicator-gtk3 patchelf
```

Toolchains:

```bash
# Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
# Node deps (Tauri CLI)
npm install
```

## Run (development)

```bash
npm run dev        # = tauri dev
```

The window starts hidden; the app lives in your tray. Left-click the tray icon (or
the tray menu → *Open dashboard*) to open the window.

> **Tip:** to see a break quickly, set the work interval to 1 min in Settings.

## Build (release binary)

```bash
npm run build      # = tauri build
```

The bundled binary and packages land in `src-tauri/target/release/`.

## Data location

- History: `~/.local/share/screenblocker/history.json`
- Settings: `~/.config/screenblocker/settings.json`

`history.json` is an append-only log of `successful | skipped | meeting_notified`
events; the dashboard aggregates it by day.

## Notes / limitations

- Wayland cannot truly *lock you out*; the overlay is a fullscreen always-on-top
  nudge, not an unescapable kiosk. That's intentional for a self-discipline tool.
- Meeting auto-detect keys off active PipeWire capture streams. Music playback does
  **not** count (only input/capture). Use the manual override for edge cases.

## Project layout

```
src/                 frontend (HTML/CSS/JS): dashboard + overlay
src-tauri/src/
  store.rs           JSON settings + history + dashboard aggregation
  timer.rs           focus/break state machine (pure, unit-tested)
  meeting.rs         PipeWire capture detection
  tray_icon.rs       7-segment countdown icon renderer
  messages.rs        stretch / meeting prompt banks
  commands.rs        Tauri commands + shared state
  lib.rs             tray, windows, 1-second cycle loop
docs/superpowers/    design spec + implementation plan
```
