use std::ffi::c_void;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows::Win32::Foundation::{BOOL, HWND, LPARAM, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWA_WINDOW_CORNER_PREFERENCE,
    DWMWCP_ROUND,
};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, InvalidateRect, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowLongPtrW, GetWindowRect, IsIconic, IsWindowVisible, PostMessageW,
    SetForegroundWindow, SetWindowLongPtrW, SetWindowPos, ShowWindow, GWL_EXSTYLE, HWND_NOTOPMOST,
    HWND_TOPMOST, SWP_FRAMECHANGED, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SW_HIDE, SW_RESTORE,
    SW_SHOW, WM_CLOSE, WM_NULL, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW,
};

/// Extracts the raw Win32 `HWND` (as `isize`) from any `HasWindowHandle` provider.
pub fn extract_hwnd(window_handle_provider: &dyn HasWindowHandle) -> Option<isize> {
    let handle = window_handle_provider.window_handle().ok()?;
    match handle.as_raw() {
        RawWindowHandle::Win32(win32) => Some(win32.hwnd.get()),
        _ => None,
    }
}

/// Applies `WS_EX_TOOLWINDOW` and strips `WS_EX_APPWINDOW` on the given Win32 `HWND`
/// so Windows never shows an icon for Lyre on the Taskbar, and enables DWM rounded corners.
pub fn apply_tool_window_style(hwnd_val: isize) {
    if hwnd_val == 0 {
        return;
    }
    let hwnd = HWND(hwnd_val as *mut c_void);
    unsafe {
        let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let new_style = (ex_style | (WS_EX_TOOLWINDOW.0 as isize)) & !(WS_EX_APPWINDOW.0 as isize);
        if new_style != ex_style {
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new_style);
            let _ = SetWindowPos(
                hwnd,
                None,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED,
            );
        }

        // Enable Windows 11 hardware-composited rounded corners & elevation shadow
        let corner_pref = DWMWCP_ROUND;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &corner_pref as *const _ as *const c_void,
            std::mem::size_of_val(&corner_pref) as u32,
        );
    }
}

/// Sets the native Windows 10/11 window title bar & border to Dark Mode or Light Mode.
pub fn apply_dark_title_bar_win32(hwnd_val: isize, dark: bool) {
    if hwnd_val == 0 {
        return;
    }
    let hwnd = HWND(hwnd_val as *mut c_void);
    let use_dark: BOOL = BOOL::from(dark);
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &use_dark as *const _ as *const c_void,
            std::mem::size_of_val(&use_dark) as u32,
        );
    }
}

/// Applies or removes TopMost (`Always on Top`) status directly via Win32 `SetWindowPos`.
pub fn set_always_on_top_win32(hwnd_val: isize, always_on_top: bool) {
    if hwnd_val == 0 {
        return;
    }
    let hwnd = HWND(hwnd_val as *mut c_void);
    let insert_after = if always_on_top {
        HWND_TOPMOST
    } else {
        HWND_NOTOPMOST
    };
    unsafe {
        let _ = SetWindowPos(hwnd, insert_after, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE);
    }
}

/// Returns `true` if the Win32 window is currently visible on screen.
pub fn is_window_visible_win32(hwnd_val: isize) -> bool {
    if hwnd_val == 0 {
        return true;
    }
    let hwnd = HWND(hwnd_val as *mut c_void);
    unsafe { IsWindowVisible(hwnd).as_bool() && !IsIconic(hwnd).as_bool() }
}

/// Shows or hides the window immediately via Win32 `ShowWindow`, brings it to the foreground
/// when shown, and invalidates its client rect so `winit`/`eframe` immediately resumes `update()`.
pub fn set_window_visible_win32(hwnd_val: isize, visible: bool) {
    if hwnd_val == 0 {
        return;
    }
    let hwnd = HWND(hwnd_val as *mut c_void);
    unsafe {
        if visible {
            if IsIconic(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            }
            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = SetForegroundWindow(hwnd);
            let _ = InvalidateRect(hwnd, None, BOOL::from(true));
            let _ = PostMessageW(hwnd, WM_NULL, WPARAM(0), LPARAM(0));
        } else {
            let _ = ShowWindow(hwnd, SW_HIDE);
        }
    }
}

/// Posts a `WM_CLOSE` message to the window (used when quitting from the system tray while hidden).
pub fn post_close_win32(hwnd_val: isize) {
    if hwnd_val == 0 {
        return;
    }
    let hwnd = HWND(hwnd_val as *mut c_void);
    unsafe {
        let _ = PostMessageW(hwnd, WM_CLOSE, WPARAM(0), LPARAM(0));
    }
}

/// Wakes up `winit`/`eframe` via `InvalidateRect` + `WM_NULL`.
pub fn wake_window_event_loop(hwnd_val: isize) {
    if hwnd_val == 0 {
        return;
    }
    let hwnd = HWND(hwnd_val as *mut c_void);
    unsafe {
        let _ = InvalidateRect(hwnd, None, BOOL::from(false));
        let _ = PostMessageW(hwnd, WM_NULL, WPARAM(0), LPARAM(0));
    }
}

/// Computes the ideal logical screen coordinates `[x, y]` for opening an external dialog
/// window of size `(dialog_w, dialog_h)` on the **exact same monitor** as `hwnd_val`,
/// centered near the main window and clamped within that monitor's work area (`rcWork`).
pub fn compute_same_monitor_dialog_pos(
    hwnd_val: isize,
    dialog_w: f32,
    dialog_h: f32,
    pixels_per_point: f32,
) -> Option<[f32; 2]> {
    if hwnd_val == 0 {
        return None;
    }
    let ppp = pixels_per_point.max(1.0);
    let hwnd = HWND(hwnd_val as *mut c_void);

    unsafe {
        let mut win_rect = RECT::default();
        if GetWindowRect(hwnd, &mut win_rect).is_err() {
            return None;
        }

        let hmon = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mut mi = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !GetMonitorInfoW(hmon, &mut mi).as_bool() {
            return None;
        }

        let work = mi.rcWork;
        let dlg_px_w = (dialog_w * ppp).round() as i32;
        let dlg_px_h = (dialog_h * ppp).round() as i32;

        let win_cx = (win_rect.left + win_rect.right) / 2;
        let win_cy = (win_rect.top + win_rect.bottom) / 2;

        let margin = 16;
        let min_x = work.left + margin;
        let max_x = (work.right - dlg_px_w - margin).max(min_x);
        let min_y = work.top + margin;
        let max_y = (work.bottom - dlg_px_h - margin).max(min_y);

        let target_x = (win_cx - dlg_px_w / 2).clamp(min_x, max_x);
        let target_y = (win_cy - dlg_px_h / 2).clamp(min_y, max_y);

        Some([target_x as f32 / ppp, target_y as f32 / ppp])
    }
}
