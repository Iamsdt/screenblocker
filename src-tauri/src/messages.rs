//! Randomised stretch / meeting prompts.

/// (title, body) pairs shown on the fullscreen break overlay.
const STRETCH: &[(&str, &str)] = &[
    (
        "Stand up & stretch",
        "Roll your shoulders back, look at something far away, and take five slow breaths. Your spine will thank you.",
    ),
    (
        "Break time — get up",
        "Reach for the ceiling, then slowly touch your toes. Hold each for ten seconds.",
    ),
    (
        "Move that body",
        "Walk to another room and back. Refill your water while you're at it.",
    ),
    (
        "Unfreeze yourself",
        "Neck rolls, wrist circles, and a big yawn. You've earned it after that focus block.",
    ),
    (
        "Eyes and spine, please",
        "Look 20 feet away for 20 seconds, then stand tall and twist gently side to side.",
    ),
    (
        "Up you get",
        "Ten slow calf raises and a good back arch. Small moves, big difference.",
    ),
];

/// Notification bodies shown instead of blocking while you're on a call.
const MEETING: &[&str] = &[
    "Straighten your back, plant both feet, and stretch your neck while you listen. No one will notice.",
    "You're on a call — stand up quietly and roll your shoulders. Stretch beats stiffness.",
    "Camera or not, rise up. Do some calf raises while they share their screen.",
    "Stay in the meeting, but get on your feet and lengthen your spine for a moment.",
    "Mic's hot, but your legs can still move. Stand and shift your weight side to side.",
];

pub fn random_stretch() -> (&'static str, &'static str) {
    STRETCH[fastrand::usize(..STRETCH.len())]
}

/// Retained for the meeting-notice message bank; not currently shown (breaks are
/// silently counted as missed during a meeting rather than notified).
#[allow(dead_code)]
pub fn random_meeting() -> &'static str {
    MEETING[fastrand::usize(..MEETING.len())]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stretch_returns_nonempty() {
        let (t, b) = random_stretch();
        assert!(!t.is_empty() && !b.is_empty());
    }

    #[test]
    fn meeting_returns_nonempty() {
        assert!(!random_meeting().is_empty());
    }
}
