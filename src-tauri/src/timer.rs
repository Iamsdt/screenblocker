//! Pure focus→break cycle state machine. No Tauri, no I/O.
//! Advanced one second at a time via [`Engine::tick`]; the caller performs
//! side effects (show overlay, log events) based on the returned [`Tick`].

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Work,
    Break,
    Paused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tick {
    /// Nothing happened this second.
    None,
    /// The work interval just elapsed. Caller decides: block (call
    /// [`Engine::start_break`]) or, if in a meeting, notify + call
    /// [`Engine::restart_work`].
    WorkEnded,
    /// The break countdown reached zero; engine has already returned to Work.
    BreakEnded,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct EngineState {
    pub phase: Phase,
    pub remaining_secs: i64,
    pub work_secs: i64,
    pub break_secs: i64,
}

#[derive(Debug, Clone)]
pub struct Engine {
    active_phase: Phase, // Work or Break (never Paused)
    paused: bool,
    remaining: i64,
    work_secs: i64,
    break_secs: i64,
}

impl Engine {
    pub fn new(work_secs: i64, break_secs: i64) -> Self {
        Engine {
            active_phase: Phase::Work,
            paused: false,
            remaining: work_secs,
            work_secs,
            break_secs,
        }
    }

    /// Apply new durations (from settings). Resets the current phase's clock.
    pub fn set_durations(&mut self, work_secs: i64, break_secs: i64) {
        self.work_secs = work_secs;
        self.break_secs = break_secs;
        self.remaining = match self.active_phase {
            Phase::Break => break_secs,
            _ => work_secs,
        };
    }

    pub fn tick(&mut self) -> Tick {
        if self.paused {
            return Tick::None;
        }
        if self.remaining > 0 {
            self.remaining -= 1;
        }
        if self.remaining > 0 {
            return Tick::None;
        }
        match self.active_phase {
            Phase::Work => {
                // Stay at Work/0 until the caller decides what to do.
                Tick::WorkEnded
            }
            Phase::Break => {
                self.active_phase = Phase::Work;
                self.remaining = self.work_secs;
                Tick::BreakEnded
            }
            // active_phase is only ever Work or Break.
            Phase::Paused => Tick::None,
        }
    }

    /// Begin the break countdown (screen will be blocked).
    pub fn start_break(&mut self) {
        self.active_phase = Phase::Break;
        self.remaining = self.break_secs;
    }

    /// Skip the current break early. Returns `Some(percent)` — how far through
    /// the break the user got (0–100) — if a break was in progress, else `None`.
    /// The caller uses the percent to decide successful vs skipped.
    pub fn skip_break_completion(&mut self) -> Option<u32> {
        if self.active_phase != Phase::Break {
            return None;
        }
        let elapsed = (self.break_secs - self.remaining).max(0);
        let percent = if self.break_secs > 0 {
            ((elapsed * 100) / self.break_secs).clamp(0, 100) as u32
        } else {
            100
        };
        self.active_phase = Phase::Work;
        self.remaining = self.work_secs;
        Some(percent)
    }

    /// Restart a fresh work interval (used when a break is skipped because a
    /// meeting is in progress).
    pub fn restart_work(&mut self) {
        self.active_phase = Phase::Work;
        self.remaining = self.work_secs;
    }

    #[allow(dead_code)]
    pub fn pause(&mut self) {
        self.paused = true;
    }
    #[allow(dead_code)]
    pub fn resume(&mut self) {
        self.paused = false;
    }
    pub fn toggle_pause(&mut self) {
        self.paused = !self.paused;
    }

    pub fn state(&self) -> EngineState {
        let phase = if self.paused {
            Phase::Paused
        } else {
            self.active_phase
        };
        EngineState {
            phase,
            remaining_secs: self.remaining,
            work_secs: self.work_secs,
            break_secs: self.break_secs,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn work_ends_after_its_duration() {
        let mut e = Engine::new(3, 2);
        assert_eq!(e.tick(), Tick::None); // 3 -> 2
        assert_eq!(e.tick(), Tick::None); // 2 -> 1
        assert_eq!(e.tick(), Tick::WorkEnded); // 1 -> 0
        assert_eq!(e.state().phase, Phase::Work);
        assert_eq!(e.state().remaining_secs, 0);
    }

    #[test]
    fn break_runs_then_returns_to_work() {
        let mut e = Engine::new(3, 2);
        e.start_break();
        assert_eq!(e.state().phase, Phase::Break);
        assert_eq!(e.tick(), Tick::None); // 2 -> 1
        assert_eq!(e.tick(), Tick::BreakEnded); // 1 -> 0
        assert_eq!(e.state().phase, Phase::Work);
        assert_eq!(e.state().remaining_secs, 3);
    }

    #[test]
    fn skip_only_counts_during_break() {
        let mut e = Engine::new(10, 5);
        assert_eq!(e.skip_break_completion(), None, "no break in progress");
        e.start_break();
        assert!(e.skip_break_completion().is_some(), "break was skipped");
        assert_eq!(e.state().phase, Phase::Work);
        assert_eq!(e.state().remaining_secs, 10);
    }

    #[test]
    fn skip_completion_reports_percent_elapsed() {
        let mut e = Engine::new(10, 100);
        e.start_break();
        // Burn 80 of 100 break seconds.
        for _ in 0..80 {
            e.tick();
        }
        assert_eq!(e.skip_break_completion(), Some(80));
    }

    #[test]
    fn skip_completion_is_zero_at_break_start() {
        let mut e = Engine::new(10, 100);
        e.start_break();
        assert_eq!(e.skip_break_completion(), Some(0));
    }

    #[test]
    fn restart_work_resets_to_a_fresh_work_interval() {
        let mut e = Engine::new(5, 2);
        e.start_break();
        e.restart_work();
        assert_eq!(e.state().phase, Phase::Work);
        assert_eq!(e.state().remaining_secs, 5);
    }

    #[test]
    fn pause_halts_the_countdown() {
        let mut e = Engine::new(5, 2);
        e.pause();
        assert_eq!(e.tick(), Tick::None);
        assert_eq!(e.tick(), Tick::None);
        assert_eq!(e.state().phase, Phase::Paused);
        assert_eq!(e.state().remaining_secs, 5, "clock did not move while paused");
        e.resume();
        assert_eq!(e.tick(), Tick::None);
        assert_eq!(e.state().remaining_secs, 4);
    }
}
