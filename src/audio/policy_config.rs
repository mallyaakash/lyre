#![allow(non_snake_case)]

use core::ffi::c_void;
use windows::core::{GUID, HRESULT, IUnknown, IUnknown_Vtbl, PCWSTR};
use windows::Win32::Media::Audio::{eCommunications, eConsole, eMultimedia, ERole};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};
use windows_core::interface;

/// Undocumented Windows COM class `CPolicyConfigClient` inside `AudioSes.dll`.
pub const CLSID_POLICY_CONFIG_CLIENT: GUID =
    GUID::from_u128(0x870af99c_171d_4f9e_af0d_e63df40c2bc9);

/// Primary `IPolicyConfig` interface used on Windows 7, 8, 10, and 11.
/// `SetDefaultEndpoint` resides at vtable slot 13 (after 3 `IUnknown` slots + 10 preceding methods).
#[interface("f8679f50-850a-41cf-9c72-430f290290c8")]
pub unsafe trait IPolicyConfig: IUnknown {
    pub unsafe fn GetMixFormat(&self, device_id: PCWSTR, format: *mut *mut c_void) -> HRESULT;
    pub unsafe fn GetDeviceFormat(
        &self,
        device_id: PCWSTR,
        default: i32,
        format: *mut *mut c_void,
    ) -> HRESULT;
    pub unsafe fn ResetDeviceFormat(&self, device_id: PCWSTR) -> HRESULT;
    pub unsafe fn SetDeviceFormat(
        &self,
        device_id: PCWSTR,
        endpoint_format: *const c_void,
        mix_format: *const c_void,
    ) -> HRESULT;
    pub unsafe fn GetProcessingPeriod(
        &self,
        device_id: PCWSTR,
        default: i32,
        default_period: *mut i64,
        min_period: *mut i64,
    ) -> HRESULT;
    pub unsafe fn SetProcessingPeriod(&self, device_id: PCWSTR, period: *const i64) -> HRESULT;
    pub unsafe fn GetShareMode(&self, device_id: PCWSTR, mode: *mut c_void) -> HRESULT;
    pub unsafe fn SetShareMode(&self, device_id: PCWSTR, mode: *const c_void) -> HRESULT;
    pub unsafe fn GetPropertyValue(
        &self,
        device_id: PCWSTR,
        fx_store: i32,
        key: *const c_void,
        value: *mut c_void,
    ) -> HRESULT;
    pub unsafe fn SetPropertyValue(
        &self,
        device_id: PCWSTR,
        fx_store: i32,
        key: *const c_void,
        value: *const c_void,
    ) -> HRESULT;
    pub unsafe fn SetDefaultEndpoint(&self, device_id: PCWSTR, role: ERole) -> HRESULT;
    pub unsafe fn SetEndpointVisibility(&self, device_id: PCWSTR, visible: i32) -> HRESULT;
}

/// Fallback `IPolicyConfigVista` interface for older or alternative COM registration paths.
#[interface("568b9108-44bf-40b4-9006-86afe5b5a620")]
pub unsafe trait IPolicyConfigVista: IUnknown {
    pub unsafe fn GetMixFormat(&self, device_id: PCWSTR, format: *mut *mut c_void) -> HRESULT;
    pub unsafe fn GetDeviceFormat(
        &self,
        device_id: PCWSTR,
        default: i32,
        format: *mut *mut c_void,
    ) -> HRESULT;
    pub unsafe fn SetDeviceFormat(
        &self,
        device_id: PCWSTR,
        endpoint_format: *const c_void,
        mix_format: *const c_void,
    ) -> HRESULT;
    pub unsafe fn GetProcessingPeriod(
        &self,
        device_id: PCWSTR,
        default: i32,
        default_period: *mut i64,
        min_period: *mut i64,
    ) -> HRESULT;
    pub unsafe fn SetProcessingPeriod(&self, device_id: PCWSTR, period: *const i64) -> HRESULT;
    pub unsafe fn GetShareMode(&self, device_id: PCWSTR, mode: *mut c_void) -> HRESULT;
    pub unsafe fn SetShareMode(&self, device_id: PCWSTR, mode: *const c_void) -> HRESULT;
    pub unsafe fn GetPropertyValue(
        &self,
        device_id: PCWSTR,
        fx_store: i32,
        key: *const c_void,
        value: *mut c_void,
    ) -> HRESULT;
    pub unsafe fn SetPropertyValue(
        &self,
        device_id: PCWSTR,
        fx_store: i32,
        key: *const c_void,
        value: *const c_void,
    ) -> HRESULT;
    pub unsafe fn SetDefaultEndpoint(&self, device_id: PCWSTR, role: ERole) -> HRESULT;
    pub unsafe fn SetEndpointVisibility(&self, device_id: PCWSTR, visible: i32) -> HRESULT;
}

/// Sets the specified Windows Audio Endpoint ID as the system default across all three roles:
/// `eConsole`, `eMultimedia`, and `eCommunications`.
pub fn set_default_endpoint_all_roles(device_id: &str) -> Result<(), String> {
    if device_id.trim().is_empty() {
        return Err("Cannot switch to an empty endpoint ID".to_string());
    }

    let wide_id: Vec<u16> = device_id.encode_utf16().chain(std::iter::once(0)).collect();
    let pcwstr = PCWSTR::from_raw(wide_id.as_ptr());
    let roles = [eConsole, eMultimedia, eCommunications];

    unsafe {
        if let Ok(policy) =
            CoCreateInstance::<_, IPolicyConfig>(&CLSID_POLICY_CONFIG_CLIENT, None, CLSCTX_ALL)
        {
            for role in roles {
                let hr = policy.SetDefaultEndpoint(pcwstr, role);
                if hr.is_err() {
                    return Err(format!(
                        "IPolicyConfig::SetDefaultEndpoint failed for role {:?}: {:?}",
                        role, hr
                    ));
                }
            }
            return Ok(());
        }

        // Fallback to IPolicyConfigVista if IPolicyConfig QueryInterface wasn't available
        let policy_vista = CoCreateInstance::<_, IPolicyConfigVista>(
            &CLSID_POLICY_CONFIG_CLIENT,
            None,
            CLSCTX_ALL,
        )
        .map_err(|e| format!("Failed to instantiate CPolicyConfigClient: {}", e))?;

        for role in roles {
            let hr = policy_vista.SetDefaultEndpoint(pcwstr, role);
            if hr.is_err() {
                return Err(format!(
                    "IPolicyConfigVista::SetDefaultEndpoint failed for role {:?}: {:?}",
                    role, hr
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::core::Interface;

    #[test]
    fn test_policy_config_guids_and_vtable_size() {
        assert_eq!(
            CLSID_POLICY_CONFIG_CLIENT,
            GUID::from_u128(0x870af99c_171d_4f9e_af0d_e63df40c2bc9)
        );
        assert_eq!(
            IPolicyConfig::IID,
            GUID::from_u128(0xf8679f50_850a_41cf_9c72_430f290290c8)
        );
        assert_eq!(
            IPolicyConfigVista::IID,
            GUID::from_u128(0x568b9108_44bf_40b4_9006_86afe5b5a620)
        );

        // Verify IPolicyConfig_Vtbl has 3 (IUnknown) + 12 methods = 15 pointers
        let ptr_size = std::mem::size_of::<usize>();
        assert_eq!(std::mem::size_of::<IPolicyConfig_Vtbl>(), 15 * ptr_size);
        // Verify IPolicyConfigVista_Vtbl has 3 (IUnknown) + 11 methods = 14 pointers
        assert_eq!(
            std::mem::size_of::<IPolicyConfigVista_Vtbl>(),
            14 * ptr_size
        );
    }
}
