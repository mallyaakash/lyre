pub mod schema;

use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::audio::{AudioSnapshot, EndpointDirection};
pub use schema::{
    format_hex_color, parse_hex_color, AppConfig, ButtonConfig, IconSource, PresetIcon, ThemeMode,
    WindowState,
};

/// Curated default accent palette for auto-seeded buttons.
pub const DEFAULT_COLOR_PALETTE: [&str; 8] = [
    "#3B82F6", // Electric Blue
    "#10B981", // Emerald Green
    "#8B5CF6", // Vivid Violet
    "#F59E0B", // Amber Gold
    "#EC4899", // Rose Pink
    "#06B6D4", // Cyan Teal
    "#F97316", // Coral Orange
    "#6366F1", // Indigo
];

/// Resolves the canonical configuration file path: `%APPDATA%\Lyre\config.json`.
pub fn config_file_path() -> PathBuf {
    if let Ok(appdata) = env::var("APPDATA") {
        let p = PathBuf::from(appdata);
        if !p.as_os_str().is_empty() {
            return p.join("Lyre").join("config.json");
        }
    }
    if let Some(base_dirs) = directories::BaseDirs::new() {
        return base_dirs.config_dir().join("Lyre").join("config.json");
    }
    PathBuf::from("config.json")
}

/// Loads `AppConfig` from `%APPDATA%/Lyre/config.json`.
/// If the file does not exist or has no buttons yet, seeds buttons from the active `AudioSnapshot`.
pub fn load_or_init_config(snapshot: &AudioSnapshot) -> AppConfig {
    let path = config_file_path();
    let mut cfg = load_from_path(&path).unwrap_or_else(|_| {
        let mut cfg = AppConfig::default();
        seed_buttons_from_snapshot(&mut cfg, snapshot);
        let _ = save_to_path(&cfg, &path);
        cfg
    });
    cfg.window.launch_at_startup = is_launch_at_startup_enabled();
    cfg
}

const STARTUP_RUN_SUBKEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const STARTUP_VALUE_NAME: &str = "Lyre";

/// Checks whether Lyre is currently registered in `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.
pub fn is_launch_at_startup_enabled() -> bool {
    use windows::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, KEY_READ,
    };
    use windows_core::PCWSTR;

    let subkey: Vec<u16> = STARTUP_RUN_SUBKEY
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let val_name: Vec<u16> = STARTUP_VALUE_NAME
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        let mut hkey = HKEY::default();
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            0,
            KEY_READ,
            &mut hkey,
        )
        .is_err()
        {
            return false;
        }
        let mut data_len: u32 = 0;
        let exists = RegQueryValueExW(
            hkey,
            PCWSTR(val_name.as_ptr()),
            None,
            None,
            None,
            Some(&mut data_len),
        )
        .is_ok()
            && data_len > 2;
        let _ = RegCloseKey(hkey);
        exists
    }
}

/// Registers or unregisters Lyre in `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.
pub fn set_launch_at_startup(enabled: bool) -> Result<(), String> {
    use windows::Win32::System::Registry::{
        RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER,
        KEY_SET_VALUE, REG_SZ,
    };
    use windows_core::PCWSTR;

    let subkey: Vec<u16> = STARTUP_RUN_SUBKEY
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let val_name: Vec<u16> = STARTUP_VALUE_NAME
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        let mut hkey = HKEY::default();
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            0,
            KEY_SET_VALUE,
            &mut hkey,
        )
        .ok()
        .map_err(|e| format!("Failed to open HKCU Run registry key: {}", e))?;

        let result = if enabled {
            let exe_path = std::env::current_exe()
                .map_err(|e| format!("Failed to resolve current executable path: {}", e))?;
            let quoted = format!("\"{}\"", exe_path.display());
            let wide_data: Vec<u16> = quoted.encode_utf16().chain(std::iter::once(0)).collect();
            let byte_slice = std::slice::from_raw_parts(
                wide_data.as_ptr() as *const u8,
                wide_data.len() * std::mem::size_of::<u16>(),
            );
            RegSetValueExW(
                hkey,
                PCWSTR(val_name.as_ptr()),
                0,
                REG_SZ,
                Some(byte_slice),
            )
            .ok()
            .map_err(|e| format!("Failed to write startup registry value: {}", e))
        } else {
            let _ = RegDeleteValueW(hkey, PCWSTR(val_name.as_ptr()));
            Ok(())
        };

        let _ = RegCloseKey(hkey);
        result
    }
}

pub fn load_from_path(path: &Path) -> Result<AppConfig, String> {
    let content =
        fs::read_to_string(path).map_err(|e| format!("Failed to read {:?}: {}", path, e))?;
    let mut cfg: AppConfig = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse {:?}: {}", path, e))?;
    cfg.window.columns = cfg.window.columns.clamp(1, 12);
    cfg.window.width = cfg.window.width.clamp(120.0, 3840.0);
    cfg.window.height = cfg.window.height.clamp(80.0, 2160.0);
    Ok(cfg)
}

/// Atomically persists `AppConfig` to `%APPDATA%/Lyre/config.json`.
pub fn save_config(cfg: &AppConfig) -> Result<(), String> {
    let path = config_file_path();
    save_to_path(cfg, &path)
}

pub fn save_to_path(cfg: &AppConfig, path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create config directory {:?}: {}", parent, e))?;
    }
    let json = serde_json::to_string_pretty(cfg)
        .map_err(|e| format!("Failed to serialize AppConfig: {}", e))?;

    let tmp_path = path.with_extension("json.tmp");
    fs::write(&tmp_path, json.as_bytes())
        .map_err(|e| format!("Failed to write temp config {:?}: {}", tmp_path, e))?;
    fs::rename(&tmp_path, path)
        .or_else(|_| fs::write(path, json.as_bytes()))
        .map_err(|e| format!("Failed to commit config {:?}: {}", path, e))?;
    Ok(())
}

/// Populates buttons from all currently active Output and Input endpoints,
/// automatically assigning smart preset icons and number badges when multiple devices
/// share the same preset icon.
pub fn seed_buttons_from_snapshot(cfg: &mut AppConfig, snapshot: &AudioSnapshot) {
    let mut all_devices = Vec::new();
    for ep in &snapshot.outputs {
        all_devices.push((ep, EndpointDirection::Output));
    }
    for ep in &snapshot.inputs {
        all_devices.push((ep, EndpointDirection::Input));
    }

    // Count how many devices map to each preset icon so we can auto-enable number badges
    // when the user has multiple devices of the same icon type (e.g., 3 Speakers or 2 DACs).
    let mut icon_totals: HashMap<PresetIcon, u32> = HashMap::new();
    for (ep, dir) in &all_devices {
        let preset = PresetIcon::infer_from_name(&ep.name, *dir);
        *icon_totals.entry(preset).or_insert(0) += 1;
    }

    let mut icon_counters: HashMap<PresetIcon, u32> = HashMap::new();

    for (idx, (ep, dir)) in all_devices.into_iter().enumerate() {
        let preset = PresetIcon::infer_from_name(&ep.name, dir);
        let seq = {
            let entry = icon_counters.entry(preset).or_insert(0);
            *entry += 1;
            *entry
        };
        let total_for_icon = icon_totals.get(&preset).copied().unwrap_or(1);

        let color_hex = DEFAULT_COLOR_PALETTE[idx % DEFAULT_COLOR_PALETTE.len()].to_string();
        let short_title = shorten_device_title(&ep.name);

        let mut btn = ButtonConfig::new(
            ep.id.clone(),
            dir,
            short_title,
            color_hex,
            IconSource::Preset(preset),
        );
        if total_for_icon > 1 {
            btn.badge_enabled = true;
            btn.badge_number = seq;
        } else {
            btn.badge_enabled = false;
            btn.badge_number = 1;
        }
        cfg.buttons.push(btn);
    }

    let count = cfg.buttons.len();
    cfg.window.columns = match count {
        0..=2 => count.max(1),
        3..=4 => count,
        5..=6 => 3,
        _ => 4,
    };
}

/// Cleans verbose Windows friendly names like `"Speakers (Realtek(R) Audio)"` or
/// `"SteelSeries Sonar - Gaming (SteelSeries Sonar Virtual Audio Device)"` into concise labels.
pub fn shorten_device_title(full_name: &str) -> String {
    let trimmed = full_name.trim();
    if let Some((prefix, rest)) = trimmed.split_once(" (") {
        let prefix = prefix.trim();
        let inside = rest
            .trim_end_matches(')')
            .replace("(R)", "")
            .replace("(TM)", "")
            .trim()
            .to_string();

        // Strip numeric Windows prefix like "2- " in "2- Arctis Nova"
        let clean_inside = inside
            .strip_prefix("2- ")
            .or_else(|| inside.strip_prefix("3- "))
            .or_else(|| inside.strip_prefix("4- "))
            .unwrap_or(&inside)
            .trim();

        let lower_prefix = prefix.to_lowercase();
        let is_generic_prefix = matches!(
            lower_prefix.as_str(),
            "speakers" | "headphones" | "microphone" | "headset earphone" | "headset microphone" | "line in" | "digital audio"
        );

        if !is_generic_prefix {
            // E.g. "SteelSeries Sonar - Gaming" -> "Sonar - Gaming" if starts with "SteelSeries "
            let concise_prefix = prefix
                .strip_prefix("SteelSeries ")
                .unwrap_or(prefix)
                .trim();
            return concise_prefix.to_string();
        } else if !clean_inside.is_empty() {
            return format!("{} ({})", prefix, clean_inside);
        }
    }
    trimmed.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::AudioEndpoint;

    #[test]
    fn test_hex_color_roundtrip() {
        let rgb = parse_hex_color("#3B82F6").expect("Should parse valid hex");
        assert_eq!(rgb, [0x3B, 0x82, 0xF6]);
        assert_eq!(format_hex_color(rgb), "#3B82F6");
        assert!(parse_hex_color("invalid").is_none());
    }

    #[test]
    fn test_config_serialization_and_seeding() {
        let snapshot = AudioSnapshot {
            outputs: vec![
                AudioEndpoint {
                    id: "{0.0.0.00000000}.{1111}".to_string(),
                    name: "Speakers (Realtek Audio)".to_string(),
                    direction: EndpointDirection::Output,
                    is_default: true,
                },
                AudioEndpoint {
                    id: "{0.0.0.00000000}.{2222}".to_string(),
                    name: "Speakers (USB Audio)".to_string(),
                    direction: EndpointDirection::Output,
                    is_default: false,
                },
            ],
            inputs: vec![AudioEndpoint {
                id: "{0.0.1.00000000}.{3333}".to_string(),
                name: "Headset Microphone (HyperX)".to_string(),
                direction: EndpointDirection::Input,
                is_default: true,
            }],
            default_output_id: Some("{0.0.0.00000000}.{1111}".to_string()),
            default_input_id: Some("{0.0.1.00000000}.{3333}".to_string()),
        };

        let mut cfg = AppConfig::default();
        seed_buttons_from_snapshot(&mut cfg, &snapshot);

        assert_eq!(cfg.buttons.len(), 3);
        // Two speakers should automatically receive badges 1 and 2
        assert!(cfg.buttons[0].badge_enabled);
        assert_eq!(cfg.buttons[0].badge_number, 1);
        assert!(cfg.buttons[1].badge_enabled);
        assert_eq!(cfg.buttons[1].badge_number, 2);
        // Single headset mic should not need a badge by default
        assert_eq!(
            cfg.buttons[2].icon,
            IconSource::Preset(PresetIcon::HeadsetMic)
        );

        let temp_file = std::env::temp_dir().join("lyre_test_config.json");
        save_to_path(&cfg, &temp_file).expect("Should save config to temp file");
        let loaded = load_from_path(&temp_file).expect("Should load config from temp file");
        assert_eq!(cfg, loaded);
        let _ = fs::remove_file(&temp_file);
    }
}
