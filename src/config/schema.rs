use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::audio::EndpointDirection;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemeMode {
    Dark,
    Light,
}

impl Default for ThemeMode {
    fn default() -> Self {
        Self::Dark
    }
}

/// Built-in scalable SVG presets for audio buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PresetIcon {
    Speakers,
    Headphones,
    IEMs,
    StudioMonitors,
    Microphone,
    HeadsetMic,
    DacAmp,
}

impl PresetIcon {
    pub const ALL: [PresetIcon; 7] = [
        PresetIcon::Speakers,
        PresetIcon::Headphones,
        PresetIcon::IEMs,
        PresetIcon::StudioMonitors,
        PresetIcon::Microphone,
        PresetIcon::HeadsetMic,
        PresetIcon::DacAmp,
    ];

    pub fn display_name(&self) -> &'static str {
        match self {
            PresetIcon::Speakers => "Speakers",
            PresetIcon::Headphones => "Headphones",
            PresetIcon::IEMs => "IEMs",
            PresetIcon::StudioMonitors => "Studio Monitors",
            PresetIcon::Microphone => "Microphone",
            PresetIcon::HeadsetMic => "Headset Mic",
            PresetIcon::DacAmp => "DAC / Amp",
        }
    }

    /// Heuristically picks a fitting preset icon from a Windows audio device's friendly name.
    pub fn infer_from_name(name: &str, direction: EndpointDirection) -> Self {
        let lower = name.to_lowercase();
        match direction {
            EndpointDirection::Input => {
                if lower.contains("headset") || lower.contains("earphone") || lower.contains("hands-free") {
                    PresetIcon::HeadsetMic
                } else {
                    PresetIcon::Microphone
                }
            }
            EndpointDirection::Output => {
                if lower.contains("iem") || lower.contains("earbud") || lower.contains("in-ear") {
                    PresetIcon::IEMs
                } else if lower.contains("dac")
                    || lower.contains("spdif")
                    || lower.contains("optical")
                    || lower.contains("digital")
                    || lower.contains("fiio")
                    || lower.contains("focusrite")
                    || lower.contains("scarlett")
                    || lower.contains("schiit")
                    || lower.contains("topping")
                {
                    PresetIcon::DacAmp
                } else if lower.contains("headphone") || lower.contains("headset") {
                    PresetIcon::Headphones
                } else if lower.contains("monitor")
                    || lower.contains("displayport")
                    || lower.contains("hdmi")
                    || lower.contains("nvidia")
                    || lower.contains("amd")
                {
                    PresetIcon::StudioMonitors
                } else {
                    PresetIcon::Speakers
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum IconSource {
    Preset(PresetIcon),
    CustomPath(PathBuf),
}

impl Default for IconSource {
    fn default() -> Self {
        IconSource::Preset(PresetIcon::Speakers)
    }
}

/// Configuration for an individual assignable audio device button.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ButtonConfig {
    pub id: String,
    pub device_id: String,
    pub direction: EndpointDirection,
    pub title: String,
    /// Hex color string `#RRGGBB`.
    /// In Light Mode: used as button fill color.
    /// In Dark Mode: used as button outline/border & active accent glow color.
    pub color_hex: String,
    pub icon: IconSource,
    /// Whether to display `Icon + Text Title` (`true`) or `Icon-Only` (`false`).
    pub show_label: bool,
    /// Whether to display the circular corner number badge.
    pub badge_enabled: bool,
    /// User-defined badge number (e.g., 1, 2, 3) to distinguish identical icons.
    pub badge_number: u32,
}

impl ButtonConfig {
    pub fn new(
        device_id: String,
        direction: EndpointDirection,
        title: String,
        color_hex: String,
        icon: IconSource,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            device_id,
            direction,
            title,
            color_hex,
            icon,
            show_label: true,
            badge_enabled: false,
            badge_number: 1,
        }
    }
}

/// Persistent window state and global layout settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WindowState {
    pub x: Option<f32>,
    pub y: Option<f32>,
    pub width: f32,
    pub height: f32,
    /// When true, strips all OS window chrome and renders strictly the tight grid of buttons.
    pub minimalist_mode: bool,
    /// Keeps the window pinned above other desktop windows.
    pub always_on_top: bool,
    /// User-configurable number of buttons per row ($N$ columns).
    pub columns: usize,
    /// Light or Dark theme mode.
    pub theme_mode: ThemeMode,
    /// Global override or default for showing labels (`true` = per-button `show_label` respected; can also bulk toggle).
    pub visible: bool,
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            x: None,
            y: None,
            width: 380.0,
            height: 220.0,
            minimalist_mode: false,
            always_on_top: true,
            columns: 3,
            theme_mode: ThemeMode::Dark,
            visible: true,
        }
    }
}

/// Root configuration persisted to `%APPDATA%/Lyre/config.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppConfig {
    pub window: WindowState,
    pub buttons: Vec<ButtonConfig>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            window: WindowState::default(),
            buttons: Vec::new(),
        }
    }
}

/// Parses a `#RRGGBB` or `RRGGBB` hex string into an `[u8; 3]` RGB triplet.
pub fn parse_hex_color(hex: &str) -> Option<[u8; 3]> {
    let trimmed = hex.trim().trim_start_matches('#');
    if trimmed.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&trimmed[0..2], 16).ok()?;
    let g = u8::from_str_radix(&trimmed[2..4], 16).ok()?;
    let b = u8::from_str_radix(&trimmed[4..6], 16).ok()?;
    Some([r, g, b])
}

/// Formats an `[u8; 3]` RGB triplet into an uppercase `#RRGGBB` string.
pub fn format_hex_color(rgb: [u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2])
}
