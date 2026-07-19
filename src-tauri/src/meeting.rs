//! Meeting detection via PipeWire. A meeting is inferred when some application
//! is actively capturing the microphone or camera.

use serde_json::Value;
use std::process::Command;

/// True if `pw-dump` output shows an active audio/video capture stream.
///
/// PipeWire represents an app capturing input as a Node whose
/// `info.props["media.class"]` is `Stream/Input/Audio` (mic) or
/// `Stream/Input/Video` (camera). We additionally require the node to be
/// `running` so idle/suspended devices don't count.
pub fn parse_pw_dump(json: &str) -> bool {
    let parsed: Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(_) => return false,
    };
    let Some(arr) = parsed.as_array() else {
        return false;
    };
    for obj in arr {
        let info = &obj["info"];
        let class = info["props"]["media.class"].as_str().unwrap_or("");
        let is_capture = class == "Stream/Input/Audio" || class == "Stream/Input/Video";
        if !is_capture {
            continue;
        }
        // `state` may be absent in some dumps; treat "running" as active.
        let state = info["state"].as_str().unwrap_or("");
        if state == "running" {
            return true;
        }
    }
    false
}

/// Query the live system. Returns false if `pw-dump` is unavailable.
pub fn is_capture_active() -> bool {
    match Command::new("pw-dump").output() {
        Ok(out) if out.status.success() => {
            parse_pw_dump(&String::from_utf8_lossy(&out.stdout))
        }
        _ => false,
    }
}

/// Effective meeting state given settings.
/// Manual override, when set, always wins over auto-detection.
pub fn meeting_state(manual_override: Option<bool>, auto_detect: bool) -> bool {
    if let Some(forced) = manual_override {
        return forced;
    }
    if auto_detect {
        is_capture_active()
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_running_mic_capture() {
        let json = r#"[
            {"info":{"state":"running","props":{"media.class":"Stream/Input/Audio","application.name":"zoom"}}}
        ]"#;
        assert!(parse_pw_dump(json));
    }

    #[test]
    fn detects_running_camera_capture() {
        let json = r#"[
            {"info":{"state":"running","props":{"media.class":"Stream/Input/Video"}}}
        ]"#;
        assert!(parse_pw_dump(json));
    }

    #[test]
    fn ignores_output_and_monitor_streams() {
        let json = r#"[
            {"info":{"state":"running","props":{"media.class":"Stream/Output/Audio","application.name":"spotify"}}},
            {"info":{"state":"idle","props":{"media.class":"Audio/Source"}}}
        ]"#;
        assert!(!parse_pw_dump(json));
    }

    #[test]
    fn ignores_idle_capture_stream() {
        let json = r#"[
            {"info":{"state":"suspended","props":{"media.class":"Stream/Input/Audio"}}}
        ]"#;
        assert!(!parse_pw_dump(json));
    }

    #[test]
    fn malformed_json_is_not_a_meeting() {
        assert!(!parse_pw_dump("not json"));
        assert!(!parse_pw_dump("{}"));
    }

    #[test]
    fn manual_override_wins() {
        assert!(meeting_state(Some(true), false));
        assert!(!meeting_state(Some(false), true));
    }
}
