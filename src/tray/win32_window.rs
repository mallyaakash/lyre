use std::ffi::c_void;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows::Win32::Foundation::{BOOL, HWND, LPARAM, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWA_WINDOW_CORNER_PREFERENCE,
    DWMWCP_ROUND,
};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, InvalidateRect, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetCursorPos, GetWindowLongPtrW, GetWindowRect, IsIconic, IsWindowVisible, PostMessageW,
    SetForegroundWindow, SetWindowLongPtrW, SetWindowPos, ShowWindow, GWL_EXSTYLE, HWND_NOTOPMOST,
    HWND_TOPMOST, SWP_FRAMECHANGED, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SW_HIDE, SW_RESTORE,
    SW_SHOW, WM_CLOSE, WM_NULL, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW,
};

#[link(name = "kernel32")]
extern "system" {
    fn LoadLibraryA(lp_lib_file_name: *const u8) -> *mut c_void;
    fn GetProcAddress(h_module: *mut c_void, lp_proc_name: *const u8) -> *const c_void;
}

#[link(name = "user32")]
extern "system" {
    fn CreateIconFromResourceEx(
        presbits: *const u8,
        dw_res_size: u32,
        f_icon: i32,
        dw_ver: u32,
        cx_desired: i32,
        cy_desired: i32,
        flags: u32,
    ) -> isize;
    fn SendMessageW(hwnd: *mut c_void, msg: u32, wparam: usize, lparam: isize) -> isize;
}

const WM_SETICON: u32 = 0x0080;
const ICON_SMALL: usize = 0;
const ICON_BIG: usize = 1;

/// Extracts the raw Win32 `HWND` (as `isize`) from any `HasWindowHandle` provider.
pub fn extract_hwnd(window_handle_provider: &dyn HasWindowHandle) -> Option<isize> {
    let handle = window_handle_provider.window_handle().ok()?;
    match handle.as_raw() {
        RawWindowHandle::Win32(win32) => Some(win32.hwnd.get()),
        _ => None,
    }
}

/// Enables Windows 10/11 dark mode for native Win32 popup menus (such as the System Tray menu)
/// via `uxtheme.dll` undocumented ordinals 135 (`SetPreferredAppMode`) & 136 (`FlushMenuThemes`).
pub fn enable_win32_dark_menus(dark: bool) {
    unsafe {
        let hmod = LoadLibraryA(b"uxtheme.dll\0".as_ptr());
        if !hmod.is_null() {
            // Ordinal 135: SetPreferredAppMode(int mode) -> 2 = ForceDark, 1 = AllowDark, 3 = ForceLight
            let proc_135 = GetProcAddress(hmod, 135usize as *const u8);
            if !proc_135.is_null() {
                let set_preferred_app_mode: unsafe extern "system" fn(i32) -> i32 =
                    std::mem::transmute(proc_135);
                let _ = set_preferred_app_mode(if dark { 2 } else { 3 });
            }
            // Ordinal 136: FlushMenuThemes()
            let proc_136 = GetProcAddress(hmod, 136usize as *const u8);
            if !proc_136.is_null() {
                let flush_menu_themes: unsafe extern "system" fn() = std::mem::transmute(proc_136);
                flush_menu_themes();
            }
        }
    }
}

/// Applies `WS_EX_APPWINDOW` and strips `WS_EX_TOOLWINDOW` on the given Win32 `HWND`
/// so Windows always displays Lyre on the Taskbar when visible (even in frameless Minimalist Mode),
/// sets the Taskbar icon (`ICON_BIG`) to `logo_bg.png` (no background) and the Title Bar icon
/// (`ICON_SMALL`) to `logo.png` (with background), and enables DWM rounded corners.
pub fn apply_tool_window_style(hwnd_val: isize) {
    if hwnd_val == 0 {
        return;
    }
    let hwnd = HWND(hwnd_val as *mut c_void);
    unsafe {
        let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let new_style = (ex_style | (WS_EX_APPWINDOW.0 as isize)) & !(WS_EX_TOOLWINDOW.0 as isize);
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

        // Set Taskbar icon (ICON_BIG) to logo_bg.png (no background) and Title Bar icon (ICON_SMALL) to logo.png (with background)
        apply_taskbar_and_titlebar_icons_win32(hwnd_val);
    }
}

/// Sets `ICON_BIG` (Taskbar / Alt+Tab icon) to `logo_bg.png` (transparent background) and
/// `ICON_SMALL` (Window Title Bar icon) to `logo.png` (with background) once per HWND.
pub fn apply_taskbar_and_titlebar_icons_win32(hwnd_val: isize) {
    use std::sync::OnceLock;
    if hwnd_val == 0 {
        return;
    }

    static ICONS: OnceLock<(isize, isize)> = OnceLock::new();
    let &(hicon_big, hicon_small) = ICONS.get_or_init(|| unsafe {
        let logo_bg_bytes = include_bytes!("../../logo_bg.png");
        let logo_with_bg_bytes = include_bytes!("../../logo.png");

        let big = CreateIconFromResourceEx(
            logo_bg_bytes.as_ptr(),
            logo_bg_bytes.len() as u32,
            1,
            0x00030000,
            64,
            64,
            0,
        );
        let small = CreateIconFromResourceEx(
            logo_with_bg_bytes.as_ptr(),
            logo_with_bg_bytes.len() as u32,
            1,
            0x00030000,
            24,
            24,
            0,
        );
        (big, small)
    });

    unsafe {
        if hicon_big != 0 {
            let _ = SendMessageW(hwnd_val as *mut c_void, WM_SETICON, ICON_BIG, hicon_big);
        }
        if hicon_small != 0 {
            let _ = SendMessageW(hwnd_val as *mut c_void, WM_SETICON, ICON_SMALL, hicon_small);
        }
    }
}

/// Sets the native Windows 10/11 window title bar & border to Dark Mode or Light Mode.
pub fn apply_dark_title_bar_win32(hwnd_val: isize, dark: bool) {
    enable_win32_dark_menus(dark);
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

/// Computes the ideal logical screen coordinates `[x, y]` for opening the custom
/// **Linear / Raycast Matte Obsidian** right-click context window right at the cursor,
/// clamped within the current monitor's work area (`rcWork`).
pub fn compute_same_monitor_cursor_menu_pos(
    hwnd_val: isize,
    menu_w: f32,
    menu_h: f32,
    pixels_per_point: f32,
) -> Option<[f32; 2]> {
    let ppp = pixels_per_point.max(1.0);
    unsafe {
        let mut pt = POINT::default();
        if GetCursorPos(&mut pt).is_err() {
            return None;
        }

        if hwnd_val != 0 {
            let hwnd = HWND(hwnd_val as *mut c_void);
            let hmon = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
            let mut mi = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            if GetMonitorInfoW(hmon, &mut mi).as_bool() {
                let work = mi.rcWork;
                let px_w = (menu_w * ppp).round() as i32;
                let px_h = (menu_h * ppp).round() as i32;
                let margin = 10;
                let min_x = work.left + margin;
                let max_x = (work.right - px_w - margin).max(min_x);
                let min_y = work.top + margin;
                let max_y = (work.bottom - px_h - margin).max(min_y);

                let target_x = pt.x.clamp(min_x, max_x);
                let target_y = pt.y.clamp(min_y, max_y);
                return Some([target_x as f32 / ppp, target_y as f32 / ppp]);
            }
        }

        Some([pt.x as f32 / ppp, pt.y as f32 / ppp])
    }
}
