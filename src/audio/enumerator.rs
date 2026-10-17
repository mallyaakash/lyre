use serde::{Deserialize, Serialize};
use windows::core::PWSTR;
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Media::Audio::{
    eCapture, eConsole, eRender, EDataFlow, IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator,
    DEVICE_STATE_ACTIVE,
};
use windows::Win32::System::Com::StructuredStorage::PropVariantToStringAlloc;
use windows::Win32::System::Com::{
    CoCreateInstance, CoTaskMemFree, CLSCTX_ALL, STGM_READ,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EndpointDirection {
    Output,
    Input,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioEndpoint {
    pub id: String,
    pub name: String,
    pub direction: EndpointDirection,
    pub is_default: bool,
}

#[derive(Debug, Clone, Default)]
pub struct AudioSnapshot {
    pub outputs: Vec<AudioEndpoint>,
    pub inputs: Vec<AudioEndpoint>,
    pub default_output_id: Option<String>,
    pub default_input_id: Option<String>,
}

/// Creates a fresh `IMMDeviceEnumerator` instance on the current COM-initialized thread.
pub fn create_device_enumerator() -> Result<IMMDeviceEnumerator, String> {
    unsafe {
        CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
            .map_err(|e| format!("CoCreateInstance(MMDeviceEnumerator) failed: {}", e))
    }
}

/// Queries a full snapshot of all active Output (`eRender`) and Input (`eCapture`) devices
/// along with the current default endpoint IDs.
pub fn enumerate_audio_snapshot(
    enumerator: &IMMDeviceEnumerator,
) -> Result<AudioSnapshot, String> {
    let default_output_id = get_default_endpoint_id(enumerator, eRender);
    let default_input_id = get_default_endpoint_id(enumerator, eCapture);

    let outputs = enumerate_direction(
        enumerator,
        eRender,
        EndpointDirection::Output,
        default_output_id.as_deref(),
    )?;
    let inputs = enumerate_direction(
        enumerator,
        eCapture,
        EndpointDirection::Input,
        default_input_id.as_deref(),
    )?;

    Ok(AudioSnapshot {
        outputs,
        inputs,
        default_output_id,
        default_input_id,
    })
}

pub fn get_default_endpoint_id(
    enumerator: &IMMDeviceEnumerator,
    data_flow: EDataFlow,
) -> Option<String> {
    unsafe {
        let device = enumerator
            .GetDefaultAudioEndpoint(data_flow, eConsole)
            .ok()?;
        get_device_id(&device).ok()
    }
}

fn enumerate_direction(
    enumerator: &IMMDeviceEnumerator,
    data_flow: EDataFlow,
    direction: EndpointDirection,
    default_id: Option<&str>,
) -> Result<Vec<AudioEndpoint>, String> {
    unsafe {
        let collection = enumerator
            .EnumAudioEndpoints(data_flow, DEVICE_STATE_ACTIVE)
            .map_err(|e| format!("EnumAudioEndpoints failed: {}", e))?;

        let count = collection
            .GetCount()
            .map_err(|e| format!("IMMDeviceCollection::GetCount failed: {}", e))?;

        let mut endpoints = Vec::with_capacity(count as usize);
        for idx in 0..count {
            if let Ok(device) = collection.Item(idx) {
                if let Ok(id) = get_device_id(&device) {
                    let name = get_device_friendly_name(&device)
                        .unwrap_or_else(|| format!("Audio Device {}", idx + 1));
                    let is_default = default_id.map(|d| d == id).unwrap_or(false);
                    endpoints.push(AudioEndpoint {
                        id,
                        name,
                        direction,
                        is_default,
                    });
                }
            }
        }
        Ok(endpoints)
    }
}

unsafe fn get_device_id(device: &IMMDevice) -> Result<String, String> {
    let raw_id: PWSTR = device
        .GetId()
        .map_err(|e| format!("IMMDevice::GetId failed: {}", e))?;
    if raw_id.is_null() {
        return Err("IMMDevice::GetId returned null pointer".to_string());
    }
    let id_str = raw_id
        .to_string()
        .map_err(|e| format!("Invalid UTF-16 in device ID: {}", e));
    CoTaskMemFree(Some(raw_id.0 as *const _));
    id_str
}

unsafe fn get_device_friendly_name(device: &IMMDevice) -> Option<String> {
    let store = device.OpenPropertyStore(STGM_READ).ok()?;
    let prop = store.GetValue(&PKEY_Device_FriendlyName).ok()?;
    let alloc_pwstr = PropVariantToStringAlloc(&prop).ok()?;
    if alloc_pwstr.is_null() {
        return None;
    }
    let friendly = alloc_pwstr.to_string().ok();
    CoTaskMemFree(Some(alloc_pwstr.0 as *const _));
    friendly
}
