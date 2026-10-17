pub mod enumerator;
pub mod notifier;
pub mod policy_config;
pub mod sonar;
pub mod volume;

use std::sync::Arc;
use std::time::{Duration, Instant};
use windows::Win32::Media::Audio::{
    eCapture, eRender, IMMDeviceEnumerator, IMMNotificationClient,
};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED, COINIT_MULTITHREADED};

pub use enumerator::{AudioEndpoint, AudioSnapshot, EndpointDirection};
use notifier::{AudioEndpointNotificationClient, NotificationSignal};
pub use policy_config::set_default_endpoint_all_roles;
pub use sonar::{is_sonar_virtual_name, SonarBridge, SonarRedirectionState};

/// Manages COM audio endpoint enumeration, default endpoint switching,
/// SteelSeries Sonar local API redirection (with automatic fallback on non-Sonar PCs),
/// mute control, and real-time `IMMNotificationClient` event listening.
pub struct AudioEngine {
    enumerator: IMMDeviceEnumerator,
    client: IMMNotificationClient,
    signal: Arc<NotificationSignal>,
    snapshot: AudioSnapshot,
    sonar: SonarBridge,
    sonar_state: Option<SonarRedirectionState>,
    last_sonar_poll: Instant,
}

impl AudioEngine {
    /// Initializes COM on the current thread (if not already initialized), creates the
    /// `IMMDeviceEnumerator`, registers the `IMMNotificationClient` callback, probes
    /// SteelSeries Sonar (if installed), and captures the initial audio device snapshot.
    pub fn new() -> Result<Self, String> {
        ensure_com_initialized();

        let enumerator = enumerator::create_device_enumerator()?;
        let signal = NotificationSignal::new();
        let client_impl = AudioEndpointNotificationClient::new(signal.clone());
        let client: IMMNotificationClient = client_impl.into();

        unsafe {
            enumerator
                .RegisterEndpointNotificationCallback(&client)
                .map_err(|e| format!("RegisterEndpointNotificationCallback failed: {}", e))?;
        }

        let snapshot = enumerator::enumerate_audio_snapshot(&enumerator).unwrap_or_default();
        signal.take_dirty();

        let mut sonar = SonarBridge::new();
        let sonar_state = sonar.query_redirections();

        Ok(Self {
            enumerator,
            client,
            signal,
            snapshot,
            sonar,
            sonar_state,
            last_sonar_poll: Instant::now(),
        })
    }

    /// Binds the `egui::Context` so external audio events immediately trigger a GUI repaint.
    pub fn set_egui_context(&self, ctx: egui::Context) {
        self.signal.set_egui_context(ctx);
    }

    /// Checks if `IMMNotificationClient` signaled any change (or if SteelSeries Sonar's
    /// internal redirection changed) and refreshes the snapshot if needed.
    pub fn poll_updates(&mut self) -> bool {
        let mut changed = false;
        if self.signal.take_dirty() {
            self.refresh();
            changed = true;
        } else if SonarBridge::is_installed()
            && self.last_sonar_poll.elapsed() >= Duration::from_millis(1200)
        {
            self.last_sonar_poll = Instant::now();
            let latest_sonar = self.sonar.query_redirections();
            if latest_sonar != self.sonar_state {
                self.sonar_state = latest_sonar;
                changed = true;
            }
        }
        changed
    }

    /// Forces an immediate re-enumeration of all active Output and Input devices
    /// as well as SteelSeries Sonar's active redirection state (if Sonar is running).
    pub fn refresh(&mut self) {
        ensure_com_initialized();
        if let Ok(snap) = enumerator::enumerate_audio_snapshot(&self.enumerator) {
            self.snapshot = snap;
        }
        self.sonar_state = self.sonar.query_redirections();
        self.last_sonar_poll = Instant::now();
    }

    /// Returns the latest cached `AudioSnapshot`.
    pub fn snapshot(&self) -> &AudioSnapshot {
        &self.snapshot
    }

    /// Returns the latest cached SteelSeries Sonar redirection state (if Sonar is running).
    pub fn sonar_state(&self) -> Option<&SonarRedirectionState> {
        self.sonar_state.as_ref()
    }

    /// Switches the Windows system default audio device (Render or Capture) directly via `IPolicyConfig`
    /// across all three roles (`eConsole`, `eMultimedia`, `eCommunications`) and refreshes the snapshot.
    pub fn switch_default_device(&mut self, device_id: &str) -> Result<(), String> {
        ensure_com_initialized();
        set_default_endpoint_all_roles(device_id)?;
        self.refresh();
        Ok(())
    }

    /// Returns `true` if the current Windows Default endpoint for `direction` is a
    /// `SteelSeries Sonar - *` virtual device.
    fn is_windows_default_sonar(&self, direction: EndpointDirection) -> bool {
        let def_id = match direction {
            EndpointDirection::Output => self.snapshot.default_output_id.as_deref(),
            EndpointDirection::Input => self.snapshot.default_input_id.as_deref(),
        };
        let Some(id) = def_id else {
            return false;
        };
        self.find_endpoint(id)
            .map(|ep| is_sonar_virtual_name(&ep.name))
            .unwrap_or(false)
    }

    /// Hybrid Dual-Mode Device Switcher:
    /// 1. Resolves the target endpoint by `device_id` (or fallback friendly name if its GUID changed).
    /// 2. If the target is a physical device and **SteelSeries Sonar** is active on this machine,
    ///    redirects Sonar's internal channels (`game`, `chat`, `media`, `aux` for Output; `mic` for Input)
    ///    directly to the target hardware device via Sonar's local HTTP API.
    /// 3. If SteelSeries Sonar is **not** installed or **not** running (e.g., on a laptop without Sonar),
    ///    or if the Windows Default Device is not currently routed through Sonar, switches the Windows
    ///    Default Endpoint across `eConsole`, `eMultimedia`, and `eCommunications` via `IPolicyConfig`.
    pub fn switch_button_device(
        &mut self,
        device_id: &str,
        fallback_title: &str,
        direction: EndpointDirection,
    ) -> Result<String, String> {
        ensure_com_initialized();

        // Ensure snapshot is fresh if the target device isn't currently found
        if self.find_endpoint(device_id).is_none() {
            self.refresh();
        }

        let resolved_id = if self.find_endpoint(device_id).is_some() {
            device_id.to_string()
        } else {
            let list = match direction {
                EndpointDirection::Output => &self.snapshot.outputs,
                EndpointDirection::Input => &self.snapshot.inputs,
            };
            let lower_title = fallback_title.trim().to_lowercase();
            list.iter()
                .find(|ep| {
                    let ep_lower = ep.name.to_lowercase();
                    let short_lower =
                        crate::config::shorten_device_title(&ep.name).to_lowercase();
                    short_lower == lower_title
                        || ep_lower.contains(&lower_title)
                        || lower_title.contains(&short_lower)
                })
                .map(|ep| ep.id.clone())
                .unwrap_or_else(|| device_id.to_string())
        };

        let target_is_sonar_virtual = self
            .find_endpoint(&resolved_id)
            .map(|ep| is_sonar_virtual_name(&ep.name))
            .unwrap_or_else(|| is_sonar_virtual_name(fallback_title));

        if !target_is_sonar_virtual {
            // Try redirecting SteelSeries Sonar's internal mixer to this physical hardware device first
            let sonar_redirected = match direction {
                EndpointDirection::Output => self
                    .sonar
                    .redirect_output_device(&resolved_id)
                    .unwrap_or(false),
                EndpointDirection::Input => self
                    .sonar
                    .redirect_input_device(&resolved_id)
                    .unwrap_or(false),
            };

            if sonar_redirected {
                // If Windows wasn't already routing through Sonar, switch Windows default too
                if !self.is_windows_default_sonar(direction) {
                    let _ = set_default_endpoint_all_roles(&resolved_id);
                }
                self.refresh();
                return Ok(resolved_id);
            }
        }

        // Standard Windows Core Audio (`IPolicyConfig`) path:
        // Used automatically on laptops/PCs without Sonar, or when clicking a Sonar virtual endpoint directly.
        set_default_endpoint_all_roles(&resolved_id)?;
        self.refresh();

        Ok(resolved_id)
    }

    /// Toggles Mute across all active Output (`eRender`) endpoints.
    pub fn toggle_mute_outputs(&mut self) -> Result<bool, String> {
        ensure_com_initialized();
        let muted = volume::toggle_mute_all(&self.enumerator, eRender)?;
        self.signal.notify_change();
        Ok(muted)
    }

    /// Toggles Mute across all active Input (`eCapture`) microphone endpoints.
    pub fn toggle_mute_inputs(&mut self) -> Result<bool, String> {
        ensure_com_initialized();
        let muted = volume::toggle_mute_all(&self.enumerator, eCapture)?;
        self.signal.notify_change();
        Ok(muted)
    }

    pub fn are_outputs_muted(&self) -> bool {
        volume::is_direction_muted(&self.enumerator, eRender)
    }

    pub fn are_inputs_muted(&self) -> bool {
        volume::is_direction_muted(&self.enumerator, eCapture)
    }

    /// Returns `true` if a button's bound audio endpoint (or its active direction) is currently muted.
    pub fn is_device_muted(&self, device_id: &str, direction: EndpointDirection) -> bool {
        if volume::is_endpoint_id_muted(&self.enumerator, device_id) {
            return true;
        }
        let dir_muted = match direction {
            EndpointDirection::Output => self.are_outputs_muted(),
            EndpointDirection::Input => self.are_inputs_muted(),
        };
        dir_muted && self.is_device_active(device_id, direction)
    }

    /// Returns `true` if the given endpoint ID is currently the active Output or Input device.
    /// - When SteelSeries Sonar is active and serving as the Windows Default device, this highlights
    ///   the physical hardware device that Sonar is currently routing audio to (`game` for Output, `mic` for Input).
    /// - When SteelSeries Sonar is not installed/running (e.g. on a laptop), this highlights the
    ///   Windows Default Output / Input endpoint directly.
    pub fn is_device_active(&self, device_id: &str, direction: EndpointDirection) -> bool {
        match direction {
            EndpointDirection::Output => {
                if self.is_windows_default_sonar(EndpointDirection::Output) {
                    if let Some(ref sonar_out) = self
                        .sonar_state
                        .as_ref()
                        .and_then(|s| s.active_output_device_id.clone())
                    {
                        return sonar_out == device_id;
                    }
                }
                self.snapshot
                    .default_output_id
                    .as_deref()
                    .map(|id| id == device_id)
                    .unwrap_or(false)
            }
            EndpointDirection::Input => {
                if self.is_windows_default_sonar(EndpointDirection::Input) {
                    if let Some(ref sonar_in) = self
                        .sonar_state
                        .as_ref()
                        .and_then(|s| s.active_input_device_id.clone())
                    {
                        return sonar_in == device_id;
                    }
                }
                self.snapshot
                    .default_input_id
                    .as_deref()
                    .map(|id| id == device_id)
                    .unwrap_or(false)
            }
        }
    }

    /// Finds an endpoint by ID in the current snapshot.
    pub fn find_endpoint(&self, device_id: &str) -> Option<&AudioEndpoint> {
        self.snapshot
            .outputs
            .iter()
            .chain(self.snapshot.inputs.iter())
            .find(|ep| ep.id == device_id)
    }
}

impl Drop for AudioEngine {
    fn drop(&mut self) {
        unsafe {
            let _ = self
                .enumerator
                .UnregisterEndpointNotificationCallback(&self.client);
        }
    }
}

pub fn ensure_com_initialized() {
    unsafe {
        let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        if hr.is_err() {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_engine_initialization_and_switching() {
        let mut engine = AudioEngine::new().expect("AudioEngine should initialize on Windows");
        engine.refresh();
        let snap = engine.snapshot().clone();

        // 1. Test hybrid switch_button_device (uses Sonar local API when Sonar is running,
        //    or Windows IPolicyConfig when Sonar is not installed)
        let physical_outputs: Vec<_> = snap
            .outputs
            .iter()
            .filter(|ep| {
                !is_sonar_virtual_name(&ep.name)
                    && !ep.name.to_lowercase().contains("steam")
                    && !ep.name.to_lowercase().contains("cable")
            })
            .cloned()
            .collect();

        if physical_outputs.len() >= 2 {
            // Find which physical output is currently active (via Sonar or Windows)
            let orig_active = physical_outputs
                .iter()
                .find(|ep| engine.is_device_active(&ep.id, EndpointDirection::Output))
                .cloned()
                .unwrap_or_else(|| physical_outputs[0].clone());

            let target = physical_outputs
                .iter()
                .find(|ep| ep.id != orig_active.id)
                .unwrap();

            engine
                .switch_button_device(&target.id, &target.name, EndpointDirection::Output)
                .expect("Hybrid switch_button_device should succeed");

            assert!(
                engine.is_device_active(&target.id, EndpointDirection::Output),
                "Target physical output '{}' should be active after switch_button_device",
                target.name
            );

            // Restore original active physical output
            engine
                .switch_button_device(
                    &orig_active.id,
                    &orig_active.name,
                    EndpointDirection::Output,
                )
                .expect("Restoring original physical output should succeed");

            assert!(
                engine.is_device_active(&orig_active.id, EndpointDirection::Output),
                "Original physical output '{}' should be active after restore",
                orig_active.name
            );
        }
    }
}
