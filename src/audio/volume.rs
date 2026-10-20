use windows::Win32::Foundation::BOOL;
use windows::Win32::Media::Audio::Endpoints::{IAudioEndpointVolume, IAudioMeterInformation};
use windows::Win32::Media::Audio::{
    eConsole, EDataFlow, IMMDevice, IMMDeviceEnumerator, DEVICE_STATE_ACTIVE,
};
use windows::Win32::System::Com::CLSCTX_ALL;

/// Returns `true` if the current default endpoint (or all active endpoints) for `data_flow` is muted.
pub fn is_direction_muted(enumerator: &IMMDeviceEnumerator, data_flow: EDataFlow) -> bool {
    unsafe {
        if let Ok(default_dev) = enumerator.GetDefaultAudioEndpoint(data_flow, eConsole) {
            if let Some(muted) = get_device_mute(&default_dev) {
                return muted;
            }
        }
        let Ok(collection) = enumerator.EnumAudioEndpoints(data_flow, DEVICE_STATE_ACTIVE) else {
            return false;
        };
        let Ok(count) = collection.GetCount() else {
            return false;
        };
        if count == 0 {
            return false;
        }
        for i in 0..count {
            if let Ok(dev) = collection.Item(i) {
                if get_device_mute(&dev) == Some(false) {
                    return false;
                }
            }
        }
        true
    }
}

/// Sets the mute state on all active endpoints for the given `data_flow` (`eRender` or `eCapture`).
pub fn set_mute_all(
    enumerator: &IMMDeviceEnumerator,
    data_flow: EDataFlow,
    mute: bool,
) -> Result<(), String> {
    unsafe {
        // First set mute on the current default endpoint
        if let Ok(default_dev) = enumerator.GetDefaultAudioEndpoint(data_flow, eConsole) {
            let _ = set_device_mute(&default_dev, mute);
        }

        // Also set mute across all active endpoints in that direction ("Mute All")
        let collection = enumerator
            .EnumAudioEndpoints(data_flow, DEVICE_STATE_ACTIVE)
            .map_err(|e| format!("EnumAudioEndpoints failed: {}", e))?;
        let count = collection
            .GetCount()
            .map_err(|e| format!("GetCount failed: {}", e))?;

        for i in 0..count {
            if let Ok(dev) = collection.Item(i) {
                let _ = set_device_mute(&dev, mute);
            }
        }
        Ok(())
    }
}

/// Toggles mute across all active endpoints for `data_flow` and returns the new mute state (`true` = muted).
pub fn toggle_mute_all(
    enumerator: &IMMDeviceEnumerator,
    data_flow: EDataFlow,
) -> Result<bool, String> {
    let currently_muted = is_direction_muted(enumerator, data_flow);
    let target_mute = !currently_muted;
    set_mute_all(enumerator, data_flow, target_mute)?;
    Ok(target_mute)
}

/// Checks whether a specific endpoint ID is currently muted via `IAudioEndpointVolume::GetMute`.
pub fn is_endpoint_id_muted(enumerator: &IMMDeviceEnumerator, device_id: &str) -> bool {
    let wide: Vec<u16> = device_id.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let Ok(dev) = enumerator.GetDevice(windows_core::PCWSTR(wide.as_ptr())) else {
            return false;
        };
        get_device_mute(&dev).unwrap_or(false)
    }
}

/// Sets the mute state on a specific endpoint ID via `IAudioEndpointVolume::SetMute`.
pub fn set_endpoint_id_mute(
    enumerator: &IMMDeviceEnumerator,
    device_id: &str,
    mute: bool,
) -> Result<(), String> {
    let wide: Vec<u16> = device_id.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let dev = enumerator
            .GetDevice(windows_core::PCWSTR(wide.as_ptr()))
            .map_err(|e| format!("IMMDeviceEnumerator::GetDevice failed: {}", e))?;
        set_device_mute(&dev, mute)
    }
}

/// Sets the mute state on the current default endpoint for `data_flow`.
pub fn set_default_endpoint_mute(
    enumerator: &IMMDeviceEnumerator,
    data_flow: EDataFlow,
    mute: bool,
) -> Result<(), String> {
    unsafe {
        let dev = enumerator
            .GetDefaultAudioEndpoint(data_flow, eConsole)
            .map_err(|e| format!("GetDefaultAudioEndpoint failed: {}", e))?;
        set_device_mute(&dev, mute)
    }
}

/// Gets the master volume scalar (`0.0..=1.0`) for a specific endpoint ID.
pub fn get_endpoint_id_volume_scalar(
    enumerator: &IMMDeviceEnumerator,
    device_id: &str,
) -> Option<f32> {
    let wide: Vec<u16> = device_id.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let dev = enumerator
            .GetDevice(windows_core::PCWSTR(wide.as_ptr()))
            .ok()?;
        let vol: IAudioEndpointVolume = dev.Activate(CLSCTX_ALL, None).ok()?;
        let scalar = vol.GetMasterVolumeLevelScalar().ok()?;
        Some(scalar.clamp(0.0, 1.0))
    }
}

/// Sets the master volume scalar (`0.0..=1.0`) for a specific endpoint ID and returns the new scalar.
pub fn set_endpoint_id_volume_scalar(
    enumerator: &IMMDeviceEnumerator,
    device_id: &str,
    scalar: f32,
) -> Result<f32, String> {
    let clamped = (scalar.clamp(0.0, 1.0) * 100.0).round() / 100.0;
    let wide: Vec<u16> = device_id.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let dev = enumerator
            .GetDevice(windows_core::PCWSTR(wide.as_ptr()))
            .map_err(|e| format!("IMMDeviceEnumerator::GetDevice failed: {}", e))?;
        let vol: IAudioEndpointVolume = dev
            .Activate(CLSCTX_ALL, None)
            .map_err(|e| format!("IMMDevice::Activate(IAudioEndpointVolume) failed: {}", e))?;
        vol.SetMasterVolumeLevelScalar(clamped, std::ptr::null())
            .map_err(|e| format!("SetMasterVolumeLevelScalar failed: {}", e))?;
        Ok(clamped)
    }
}

/// Adjusts the master volume scalar (`0.0..=1.0`) for a specific endpoint ID by `delta` (e.g. `±0.02`)
/// and returns the new volume scalar (`0.0..=1.0`).
pub fn adjust_endpoint_id_volume_scalar(
    enumerator: &IMMDeviceEnumerator,
    device_id: &str,
    delta: f32,
) -> Result<f32, String> {
    let current = get_endpoint_id_volume_scalar(enumerator, device_id).unwrap_or(0.5);
    let current_pct = (current * 100.0).round();
    let delta_pct = (delta * 100.0).round();
    let target_pct = (current_pct + delta_pct).clamp(0.0, 100.0);
    set_endpoint_id_volume_scalar(enumerator, device_id, target_pct / 100.0)
}

/// Reads the live audio peak meter value (`0.0..=1.0`) for a specific endpoint ID via `IAudioMeterInformation::GetPeakValue`.
pub fn get_endpoint_id_peak_value(enumerator: &IMMDeviceEnumerator, device_id: &str) -> f32 {
    let wide: Vec<u16> = device_id.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let Ok(dev) = enumerator.GetDevice(windows_core::PCWSTR(wide.as_ptr())) else {
            return 0.0;
        };
        get_device_peak(&dev).unwrap_or(0.0)
    }
}

/// Reads the live audio peak meter value (`0.0..=1.0`) for the current default endpoint in `data_flow`.
pub fn get_default_endpoint_peak_value(
    enumerator: &IMMDeviceEnumerator,
    data_flow: EDataFlow,
) -> f32 {
    unsafe {
        let Ok(dev) = enumerator.GetDefaultAudioEndpoint(data_flow, eConsole) else {
            return 0.0;
        };
        get_device_peak(&dev).unwrap_or(0.0)
    }
}

unsafe fn get_device_peak(device: &IMMDevice) -> Option<f32> {
    let meter: IAudioMeterInformation = device.Activate(CLSCTX_ALL, None).ok()?;
    let peak = meter.GetPeakValue().ok()?;
    Some(peak.clamp(0.0, 1.0))
}

unsafe fn get_device_mute(device: &IMMDevice) -> Option<bool> {
    let vol: IAudioEndpointVolume = device.Activate(CLSCTX_ALL, None).ok()?;
    let muted = vol.GetMute().ok()?;
    Some(muted.as_bool())
}

unsafe fn set_device_mute(device: &IMMDevice, mute: bool) -> Result<(), String> {
    let vol: IAudioEndpointVolume = device
        .Activate(CLSCTX_ALL, None)
        .map_err(|e| format!("IMMDevice::Activate(IAudioEndpointVolume) failed: {}", e))?;
    vol.SetMute(BOOL::from(mute), std::ptr::null())
        .map_err(|e| format!("IAudioEndpointVolume::SetMute failed: {}", e))
}
