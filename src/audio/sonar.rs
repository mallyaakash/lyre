use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

use serde::Deserialize;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SonarRedirectionState {
    pub active_output_device_id: Option<String>,
    pub active_input_device_id: Option<String>,
}

#[derive(Deserialize)]
struct CoreProps {
    #[serde(rename = "ggEncryptedAddress")]
    gg_encrypted_address: Option<String>,
}

#[derive(Deserialize)]
struct SubAppsRoot {
    #[serde(rename = "subApps")]
    sub_apps: Option<SubAppsMap>,
}

#[derive(Deserialize)]
struct SubAppsMap {
    sonar: Option<SonarSubApp>,
}

#[derive(Deserialize)]
struct SonarSubApp {
    #[serde(rename = "isRunning", default)]
    is_running: bool,
    metadata: Option<SonarMetadata>,
}

#[derive(Deserialize)]
struct SonarMetadata {
    #[serde(rename = "webServerAddress")]
    web_server_address: Option<String>,
}

#[derive(Deserialize)]
struct ClassicRedirectionItem {
    id: String,
    #[serde(rename = "deviceId")]
    device_id: String,
}

#[derive(Deserialize)]
struct StreamRedirectionItem {
    #[serde(rename = "streamRedirectionId")]
    stream_redirection_id: String,
    #[serde(rename = "deviceId")]
    device_id: String,
}

/// Returns `true` if an endpoint friendly name or button title refers to one of SteelSeries Sonar's
/// virtual audio endpoints (rather than a physical hardware audio device).
pub fn is_sonar_virtual_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.contains("steelseries sonar") || lower.starts_with("sonar -")
}

/// Lightweight, pure-Rust client for SteelSeries Sonar's local Kestrel HTTP server.
/// Automatically falls back to a no-op on systems where SteelSeries GG / Sonar is not installed
/// (such as a laptop without Sonar).
pub struct SonarBridge {
    cached_addr: Option<SocketAddr>,
    last_discovery_attempt: Option<Instant>,
}

impl Default for SonarBridge {
    fn default() -> Self {
        Self::new()
    }
}

impl SonarBridge {
    pub fn new() -> Self {
        let mut bridge = Self {
            cached_addr: None,
            last_discovery_attempt: None,
        };
        let _ = bridge.ensure_addr();
        bridge
    }

    fn core_props_path() -> PathBuf {
        let program_data =
            std::env::var("PROGRAMDATA").unwrap_or_else(|_| "C:\\ProgramData".to_string());
        PathBuf::from(program_data)
            .join("SteelSeries")
            .join("SteelSeries Engine 3")
            .join("coreProps.json")
    }

    /// Checks whether SteelSeries GG is installed on this machine (`coreProps.json` exists).
    pub fn is_installed() -> bool {
        Self::core_props_path().exists()
    }

    /// Resolves and caches the local `127.0.0.1:<port>` address of SteelSeries Sonar's Kestrel web server.
    /// Returns `None` immediately (zero overhead) on machines without SteelSeries GG installed.
    fn ensure_addr(&mut self) -> Option<SocketAddr> {
        if let Some(addr) = self.cached_addr {
            return Some(addr);
        }

        let core_props_file = Self::core_props_path();
        if !core_props_file.exists() {
            return None;
        }

        // Throttle re-discovery attempts if SteelSeries GG is installed but currently closed
        if let Some(last) = self.last_discovery_attempt {
            if last.elapsed() < Duration::from_secs(4) {
                return None;
            }
        }
        self.last_discovery_attempt = Some(Instant::now());

        let raw = std::fs::read_to_string(&core_props_file).ok()?;
        let props: CoreProps = serde_json::from_str(&raw).ok()?;
        let gg_addr = props.gg_encrypted_address?;
        if gg_addr.trim().is_empty() {
            return None;
        }

        let url = format!("https://{}/subApps", gg_addr.trim());
        let output = Command::new("curl.exe")
            .args(["-s", "-k", "--max-time", "1", &url])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let root: SubAppsRoot = serde_json::from_slice(&output.stdout).ok()?;
        let sonar = root.sub_apps?.sonar?;
        let web_addr = sonar.metadata?.web_server_address?;
        if web_addr.trim().is_empty() && !sonar.is_running {
            return None;
        }

        let stripped = web_addr
            .trim()
            .trim_start_matches("http://")
            .trim_start_matches("https://")
            .trim_end_matches('/');

        let socket_addr: SocketAddr = stripped.parse().ok()?;
        self.cached_addr = Some(socket_addr);
        Some(socket_addr)
    }

    /// Executes a sub-millisecond HTTP/1.1 request against Sonar's local loopback Kestrel server.
    /// If the cached port fails (e.g. SteelSeries GG restarted), invalidates the cache and retries once.
    fn request_http(&mut self, method: &str, path: &str) -> Option<(u16, String)> {
        let addr = self.ensure_addr()?;
        if let Some(res) = Self::send_raw_http(addr, method, path) {
            return Some(res);
        }

        // Cached port may be stale after a SteelSeries GG restart; force re-discovery once
        self.cached_addr = None;
        self.last_discovery_attempt = None;
        let new_addr = self.ensure_addr()?;
        Self::send_raw_http(new_addr, method, path)
    }

    fn send_raw_http(addr: SocketAddr, method: &str, path: &str) -> Option<(u16, String)> {
        let mut stream = TcpStream::connect_timeout(&addr, Duration::from_millis(250)).ok()?;
        let _ = stream.set_read_timeout(Some(Duration::from_millis(350)));
        let _ = stream.set_write_timeout(Some(Duration::from_millis(250)));

        let req = format!(
            "{method} {path} HTTP/1.1\r\nHost: {addr}\r\nAccept: application/json\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        );
        stream.write_all(req.as_bytes()).ok()?;
        stream.flush().ok()?;

        let mut buf = Vec::with_capacity(1024);
        stream.read_to_end(&mut buf).ok()?;
        let text = String::from_utf8_lossy(&buf);

        let (headers, body) = text.split_once("\r\n\r\n")?;
        let status_line = headers.lines().next()?;
        let status_code: u16 = status_line.split_whitespace().nth(1)?.parse().ok()?;

        Some((status_code, body.trim().to_string()))
    }

    fn encode_device_id(device_id: &str) -> String {
        device_id.replace('{', "%7B").replace('}', "%7D")
    }

    /// Queries SteelSeries Sonar for the currently active physical Output (`game` / `monitoring`)
    /// and Input (`mic`) redirection device IDs.
    /// Returns `None` if SteelSeries Sonar is not installed or not running.
    pub fn query_redirections(&mut self) -> Option<SonarRedirectionState> {
        let (status, mode_body) = self.request_http("GET", "/mode")?;
        if status != 200 {
            return None;
        }
        let mode = mode_body.trim_matches('"').trim();

        if mode.eq_ignore_ascii_case("stream") {
            let (st, body) = self.request_http("GET", "/streamRedirections")?;
            if st != 200 {
                return None;
            }
            let items: Vec<StreamRedirectionItem> = serde_json::from_str(&body).ok()?;
            let out_id = items
                .iter()
                .find(|i| i.stream_redirection_id == "monitoring" && !i.device_id.is_empty())
                .or_else(|| {
                    items
                        .iter()
                        .find(|i| i.stream_redirection_id == "streaming" && !i.device_id.is_empty())
                })
                .map(|i| i.device_id.clone());
            let in_id = items
                .iter()
                .find(|i| i.stream_redirection_id == "mic" && !i.device_id.is_empty())
                .map(|i| i.device_id.clone());

            return Some(SonarRedirectionState {
                active_output_device_id: out_id,
                active_input_device_id: in_id,
            });
        }

        let (st, body) = self.request_http("GET", "/classicRedirections")?;
        if st != 200 {
            return None;
        }
        let items: Vec<ClassicRedirectionItem> = serde_json::from_str(&body).ok()?;
        let out_id = items
            .iter()
            .find(|i| i.id == "game" && !i.device_id.is_empty())
            .or_else(|| items.iter().find(|i| i.id == "media" && !i.device_id.is_empty()))
            .or_else(|| items.iter().find(|i| i.id == "chat" && !i.device_id.is_empty()))
            .map(|i| i.device_id.clone());
        let in_id = items
            .iter()
            .find(|i| i.id == "mic" && !i.device_id.is_empty())
            .map(|i| i.device_id.clone());

        Some(SonarRedirectionState {
            active_output_device_id: out_id,
            active_input_device_id: in_id,
        })
    }

    /// Redirects all SteelSeries Sonar Output channels (`game`, `chat`, `media`, `aux`, plus Streamer
    /// `monitoring` if in stream mode) to `device_id`.
    /// Returns `Ok(true)` if Sonar handled the switch, or `Ok(false)` if Sonar is not installed/running.
    pub fn redirect_output_device(&mut self, device_id: &str) -> Result<bool, String> {
        if self.ensure_addr().is_none() {
            return Ok(false);
        }

        let encoded = Self::encode_device_id(device_id);
        let mut any_succeeded = false;

        // Always update all 4 Classic channels so Game, Chat, Media, and Aux stay unified
        for channel in ["game", "chat", "media", "aux"] {
            let path = format!("/classicRedirections/{channel}/deviceId/{encoded}");
            if let Some((status, _)) = self.request_http("PUT", &path) {
                if (200..300).contains(&status) {
                    any_succeeded = true;
                }
            }
        }

        // If user happens to have Sonar in Streamer mode, also redirect monitoring & streaming
        if let Some((200, mode_body)) = self.request_http("GET", "/mode") {
            if mode_body.trim_matches('"').eq_ignore_ascii_case("stream") {
                for stream_ch in ["monitoring", "streaming"] {
                    let path = format!("/streamRedirections/{stream_ch}/deviceId/{encoded}");
                    if let Some((status, _)) = self.request_http("PUT", &path) {
                        if (200..300).contains(&status) {
                            any_succeeded = true;
                        }
                    }
                }
            }
        }

        Ok(any_succeeded)
    }

    /// Redirects SteelSeries Sonar's Microphone channel (`mic`) to `device_id`.
    /// Returns `Ok(true)` if Sonar handled the switch, or `Ok(false)` if Sonar is not installed/running.
    pub fn redirect_input_device(&mut self, device_id: &str) -> Result<bool, String> {
        if self.ensure_addr().is_none() {
            return Ok(false);
        }

        let encoded = Self::encode_device_id(device_id);
        let mut any_succeeded = false;

        let classic_path = format!("/classicRedirections/mic/deviceId/{encoded}");
        if let Some((status, _)) = self.request_http("PUT", &classic_path) {
            if (200..300).contains(&status) {
                any_succeeded = true;
            }
        }

        if let Some((200, mode_body)) = self.request_http("GET", "/mode") {
            if mode_body.trim_matches('"').eq_ignore_ascii_case("stream") {
                let stream_path = format!("/streamRedirections/mic/deviceId/{encoded}");
                if let Some((status, _)) = self.request_http("PUT", &stream_path) {
                    if (200..300).contains(&status) {
                        any_succeeded = true;
                    }
                }
            }
        }

        Ok(any_succeeded)
    }
}
