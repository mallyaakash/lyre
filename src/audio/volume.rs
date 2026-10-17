use windows::Win32::Foundation::BOOL;
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
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
