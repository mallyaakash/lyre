#![allow(non_snake_case)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use windows::core::{implement, PCWSTR};
use windows::Win32::Media::Audio::{
    EDataFlow, ERole, IMMNotificationClient, IMMNotificationClient_Impl, DEVICE_STATE,
};
use windows::Win32::UI::Shell::PropertiesSystem::PROPERTYKEY;

/// Shared notification state signaled by `IMMNotificationClient` callbacks.
#[derive(Default)]
pub struct NotificationSignal {
    pub dirty: AtomicBool,
    pub egui_ctx: Mutex<Option<egui::Context>>,
}

impl NotificationSignal {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            dirty: AtomicBool::new(true),
            egui_ctx: Mutex::new(None),
        })
    }

    pub fn set_egui_context(&self, ctx: egui::Context) {
        if let Ok(mut guard) = self.egui_ctx.lock() {
            *guard = Some(ctx);
        }
    }

    pub fn notify_change(&self) {
        self.dirty.store(true, Ordering::SeqCst);
        if let Ok(guard) = self.egui_ctx.lock() {
            if let Some(ctx) = guard.as_ref() {
                ctx.request_repaint();
            }
        }
    }

    pub fn take_dirty(&self) -> bool {
        self.dirty.swap(false, Ordering::SeqCst)
    }
}

#[implement(IMMNotificationClient)]
pub struct AudioEndpointNotificationClient {
    signal: Arc<NotificationSignal>,
}

impl AudioEndpointNotificationClient {
    pub fn new(signal: Arc<NotificationSignal>) -> Self {
        Self { signal }
    }
}

impl IMMNotificationClient_Impl for AudioEndpointNotificationClient_Impl {
    fn OnDeviceStateChanged(
        &self,
        _pwstrdeviceid: &PCWSTR,
        _dwnewstate: DEVICE_STATE,
    ) -> windows::core::Result<()> {
        self.signal.notify_change();
        Ok(())
    }

    fn OnDeviceAdded(&self, _pwstrdeviceid: &PCWSTR) -> windows::core::Result<()> {
        self.signal.notify_change();
        Ok(())
    }

    fn OnDeviceRemoved(&self, _pwstrdeviceid: &PCWSTR) -> windows::core::Result<()> {
        self.signal.notify_change();
        Ok(())
    }

    fn OnDefaultDeviceChanged(
        &self,
        _flow: EDataFlow,
        _role: ERole,
        _pwstrdefaultdeviceid: &PCWSTR,
    ) -> windows::core::Result<()> {
        self.signal.notify_change();
        Ok(())
    }

    fn OnPropertyValueChanged(
        &self,
        _pwstrdeviceid: &PCWSTR,
        _key: &PROPERTYKEY,
    ) -> windows::core::Result<()> {
        self.signal.notify_change();
        Ok(())
    }
}
