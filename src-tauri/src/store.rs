//! Persistent JSON storage: settings + append-only break-history log,
//! plus dashboard aggregation.

use chrono::{DateTime, Duration, Local, NaiveDate};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Minimum successful breaks in a day for it to count toward the streak.
const STREAK_MIN_SUCCESS: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    Successful,
    Skipped,
    MeetingNotified,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub ts: String,
    #[serde(rename = "type")]
    pub kind: EventType,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct History {
    events: Vec<Event>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub work_minutes: u32,
    pub break_minutes: u32,
    pub auto_detect_meetings: bool,
    /// `None` = follow auto-detect; `Some(true/false)` = forced on/off.
    pub manual_meeting_override: Option<bool>,
    pub start_on_login: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            work_minutes: 25,
            break_minutes: 5,
            auto_detect_meetings: true,
            manual_meeting_override: None,
            start_on_login: true,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DayCount {
    pub date: String, // YYYY-MM-DD
    pub successful: u32,
    pub skipped: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct DashboardData {
    pub today_successful: u32,
    pub today_skipped: u32,
    pub streak: u32,
    pub all_time_total: u32,
    pub days: Vec<DayCount>,
}

pub struct Store {
    history_path: PathBuf,
    settings_path: PathBuf,
}

impl Store {
    /// Production paths under the user's XDG data/config dirs.
    pub fn new() -> Self {
        let dirs = ProjectDirs::from("com", "shudiptot", "screenblocker");
        let (data_dir, config_dir) = match dirs {
            Some(d) => (d.data_local_dir().to_path_buf(), d.config_dir().to_path_buf()),
            None => (PathBuf::from("."), PathBuf::from(".")),
        };
        Store {
            history_path: data_dir.join("history.json"),
            settings_path: config_dir.join("settings.json"),
        }
    }

    /// Explicit paths — used by tests.
    #[allow(dead_code)]
    pub fn with_paths(history: PathBuf, settings: PathBuf) -> Self {
        Store { history_path: history, settings_path: settings }
    }

    pub fn load_settings(&self) -> Settings {
        fs::read_to_string(&self.settings_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save_settings(&self, settings: &Settings) -> io::Result<()> {
        let json = serde_json::to_string_pretty(settings).unwrap();
        write_atomic(&self.settings_path, json.as_bytes())
    }

    fn load_history(&self) -> History {
        fs::read_to_string(&self.history_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn save_history(&self, history: &History) -> io::Result<()> {
        let json = serde_json::to_string(history).unwrap();
        write_atomic(&self.history_path, json.as_bytes())
    }

    pub fn append_event(&self, kind: EventType) -> io::Result<()> {
        self.append_event_at(kind, Local::now())
    }

    /// Append with an explicit timestamp (deterministic in tests).
    pub fn append_event_at(&self, kind: EventType, at: DateTime<Local>) -> io::Result<()> {
        let mut history = self.load_history();
        history.events.push(Event { ts: at.to_rfc3339(), kind });
        self.save_history(&history)
    }

    pub fn dashboard_data(&self, days: usize) -> DashboardData {
        self.dashboard_data_asof(days, Local::now().date_naive())
    }

    /// Aggregation with an explicit "today" (deterministic in tests).
    pub fn dashboard_data_asof(&self, days: usize, today: NaiveDate) -> DashboardData {
        let history = self.load_history();

        // date -> (successful, skipped)
        use std::collections::HashMap;
        let mut per_day: HashMap<NaiveDate, (u32, u32)> = HashMap::new();
        let mut all_time_total = 0u32;

        for ev in &history.events {
            let date = match DateTime::parse_from_rfc3339(&ev.ts) {
                Ok(dt) => dt.with_timezone(&Local).date_naive(),
                Err(_) => continue,
            };
            let entry = per_day.entry(date).or_insert((0, 0));
            match ev.kind {
                EventType::Successful => {
                    entry.0 += 1;
                    all_time_total += 1;
                }
                EventType::Skipped => entry.1 += 1,
                EventType::MeetingNotified => {}
            }
        }

        let (today_successful, today_skipped) =
            per_day.get(&today).copied().unwrap_or((0, 0));

        // Streak: consecutive days ending today meeting the threshold.
        let mut streak = 0u32;
        let mut cursor = today;
        loop {
            let s = per_day.get(&cursor).map(|(s, _)| *s).unwrap_or(0);
            if s >= STREAK_MIN_SUCCESS {
                streak += 1;
                cursor -= Duration::days(1);
            } else {
                break;
            }
        }

        // Last `days` window, ascending, including today.
        let mut day_counts = Vec::with_capacity(days);
        for i in (0..days).rev() {
            let d = today - Duration::days(i as i64);
            let (s, k) = per_day.get(&d).copied().unwrap_or((0, 0));
            day_counts.push(DayCount {
                date: d.format("%Y-%m-%d").to_string(),
                successful: s,
                skipped: k,
            });
        }

        DashboardData {
            today_successful,
            today_skipped,
            streak,
            all_time_total,
            days: day_counts,
        }
    }
}

impl Default for Store {
    fn default() -> Self {
        Self::new()
    }
}

fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn temp_store(tag: &str) -> Store {
        let mut base = std::env::temp_dir();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        base.push(format!("sb-test-{tag}-{nanos}"));
        std::fs::create_dir_all(&base).unwrap();
        Store::with_paths(base.join("history.json"), base.join("settings.json"))
    }

    fn at(y: i32, m: u32, d: u32) -> DateTime<Local> {
        Local.with_ymd_and_hms(y, m, d, 12, 0, 0).unwrap()
    }

    #[test]
    fn settings_roundtrip_and_default() {
        let store = temp_store("settings");
        assert_eq!(store.load_settings().work_minutes, 25);
        let mut s = Settings::default();
        s.work_minutes = 40;
        s.manual_meeting_override = Some(true);
        store.save_settings(&s).unwrap();
        let loaded = store.load_settings();
        assert_eq!(loaded.work_minutes, 40);
        assert_eq!(loaded.manual_meeting_override, Some(true));
    }

    #[test]
    fn aggregates_by_day_with_streak_and_totals() {
        let store = temp_store("agg");
        let today = at(2026, 7, 19);
        // today: 2 successful, 1 skipped
        store.append_event_at(EventType::Successful, today).unwrap();
        store.append_event_at(EventType::Successful, today).unwrap();
        store.append_event_at(EventType::Skipped, today).unwrap();
        // yesterday: 1 successful
        store.append_event_at(EventType::Successful, at(2026, 7, 18)).unwrap();
        // 2 days ago: only a meeting notice (breaks the streak, not a break)
        store.append_event_at(EventType::MeetingNotified, at(2026, 7, 17)).unwrap();
        // 3 days ago: 1 successful (isolated, before the gap)
        store.append_event_at(EventType::Successful, at(2026, 7, 16)).unwrap();

        let data = store.dashboard_data_asof(14, today.date_naive());
        assert_eq!(data.today_successful, 2);
        assert_eq!(data.today_skipped, 1);
        assert_eq!(data.streak, 2, "today + yesterday, broken by the meeting-only day");
        assert_eq!(data.all_time_total, 4, "successful breaks all-time");
        assert_eq!(data.days.len(), 14);
        let last = data.days.last().unwrap();
        assert_eq!(last.date, "2026-07-19");
        assert_eq!(last.successful, 2);
        assert_eq!(last.skipped, 1);
    }
}
