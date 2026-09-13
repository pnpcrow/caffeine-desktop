//! Persistent user settings (JSON in %APPDATA%/caffeine-desktop/settings.json).
//! Schema is shared with the Flutter GUI.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// How the blackout overlay is dismissed via keyboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UnlockKey {
    /// Any key press dismisses.
    Any,
    /// Only ESC dismisses.
    Esc,
    /// Only Space dismisses.
    Space,
    /// Only Enter dismisses.
    Enter,
}

/// How the blackout overlay is dismissed via mouse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UnlockMouse {
    /// Mouse never dismisses (keyboard only). Recommended while an AI
    /// Computer-Use agent is driving the pointer.
    Off,
    /// Any pointer movement beyond a small threshold dismisses.
    Move,
    /// A deliberate side-to-side "shake" dismisses. AI-friendly: straight
    /// synthetic mouse paths almost never match a shake pattern.
    Shake,
    /// Any mouse button click dismisses.
    Click,
}

/// Where the status OSD is anchored on the primary monitor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OsdPosition {
    TopLeft,
    TopCenter,
    TopRight,
    MiddleLeft,
    Center,
    MiddleRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// Keep display/system awake via SetThreadExecutionState.
    pub awake_enabled: bool,
    /// Auto blackout after N idle seconds (toggle).
    pub auto_blackout_enabled: bool,
    /// Idle seconds before auto blackout.
    pub auto_blackout_secs: u64,
    /// Keyboard dismiss method.
    pub unlock_key: UnlockKey,
    /// Mouse dismiss method.
    pub unlock_mouse: UnlockMouse,
    /// Start minimized to tray (settings window hidden).
    pub start_minimized: bool,
    /// Show the tiny always-on-top status OSD (capture-excluded).
    #[serde(default = "default_osd_enabled")]
    pub osd_enabled: bool,
    /// OSD anchor position (9-way).
    #[serde(default = "default_osd_position")]
    pub osd_position: OsdPosition,
}

fn default_osd_enabled() -> bool {
    true
}

fn default_osd_position() -> OsdPosition {
    OsdPosition::TopRight
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            awake_enabled: true,
            auto_blackout_enabled: true,
            auto_blackout_secs: 300,
            unlock_key: UnlockKey::Esc,
            unlock_mouse: UnlockMouse::Shake,
            start_minimized: false,
            osd_enabled: true,
            osd_position: OsdPosition::TopRight,
        }
    }
}

impl Settings {
    pub fn file_path() -> PathBuf {
        let base = std::env::var("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."));
        base.join("caffeine-desktop").join("settings.json")
    }

    pub fn load() -> Self {
        let path = Self::file_path();
        match fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str::<Settings>(&text).unwrap_or_default(),
            Err(_) => Settings::default(),
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::file_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(&path, text).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_product_spec() {
        let s = Settings::default();
        assert!(s.awake_enabled);
        assert!(s.auto_blackout_enabled);
        assert_eq!(s.auto_blackout_secs, 300);
        assert_eq!(s.unlock_key, UnlockKey::Esc);
        assert_eq!(s.unlock_mouse, UnlockMouse::Shake);
        assert!(!s.start_minimized);
        assert!(s.osd_enabled);
        assert_eq!(s.osd_position, OsdPosition::TopRight);
    }

    #[test]
    fn json_round_trip_uses_lowercase_enums() {
        let mut s = Settings::default();
        s.unlock_key = UnlockKey::Space;
        s.unlock_mouse = UnlockMouse::Click;
        s.osd_position = OsdPosition::MiddleLeft;
        let text = serde_json::to_string(&s).unwrap();
        assert!(text.contains("\"space\""), "{text}");
        assert!(text.contains("\"click\""), "{text}");
        assert!(text.contains("\"middle_left\""), "{text}");
        let back: Settings = serde_json::from_str(&text).unwrap();
        assert_eq!(back.unlock_key, UnlockKey::Space);
        assert_eq!(back.osd_position, OsdPosition::MiddleLeft);
        // Unknown/corrupt files fall back to defaults, never crash.
        let fallback: Settings =
            serde_json::from_str("{broken").unwrap_or_default();
        assert!(fallback.awake_enabled);
    }

    #[test]
    fn pre_osd_settings_file_still_loads() {
        // settings.json written by <=0.1.0 has no OSD keys: every legacy
        // value must survive and the OSD knobs fall back to their defaults.
        let legacy = serde_json::json!({
            "awake_enabled": false,
            "auto_blackout_enabled": false,
            "auto_blackout_secs": 120,
            "unlock_key": "enter",
            "unlock_mouse": "off",
            "start_minimized": true
        })
        .to_string();
        let s: Settings = serde_json::from_str(&legacy).unwrap();
        assert!(!s.awake_enabled);
        assert!(!s.auto_blackout_enabled);
        assert_eq!(s.auto_blackout_secs, 120);
        assert_eq!(s.unlock_key, UnlockKey::Enter);
        assert_eq!(s.unlock_mouse, UnlockMouse::Off);
        assert!(s.start_minimized);
        assert!(s.osd_enabled);
        assert_eq!(s.osd_position, OsdPosition::TopRight);
    }
}
