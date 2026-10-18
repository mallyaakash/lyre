pub mod win32_window;

use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};

use muda::{
    CheckMenuItem, ContextMenu, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem,
};
use tray_icon::{
    Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
};

pub const MENU_SHOW_HIDE: &str = "lyre_show_hide";
pub const MENU_ALWAYS_ON_TOP: &str = "lyre_always_on_top";
pub const MENU_OPEN_CONFIG: &str = "lyre_open_config";
pub const MENU_QUIT: &str = "lyre_quit";

pub const CTX_SWITCH_BTN: &str = "lyre_ctx_switch_btn";
pub const CTX_EDIT_BTN: &str = "lyre_ctx_edit_btn";
pub const CTX_DUPLICATE_BTN: &str = "lyre_ctx_duplicate_btn";
pub const CTX_DELETE_BTN: &str = "lyre_ctx_delete_btn";
pub const CTX_ADD_BTN: &str = "lyre_ctx_add_btn";
pub const CTX_MUTE_OUTPUTS: &str = "lyre_ctx_mute_outputs";
pub const CTX_MUTE_INPUTS: &str = "lyre_ctx_mute_inputs";
pub const CTX_TOGGLE_MINIMALIST: &str = "lyre_ctx_toggle_minimalist";
pub const CTX_TOGGLE_THEME: &str = "lyre_ctx_toggle_theme";
pub const CTX_TOGGLE_LABELS: &str = "lyre_ctx_toggle_labels";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayCommand {
    SetVisibility(bool),
    SetAlwaysOnTop(bool),
    OpenConfigMode,
    Quit,
    ContextSwitchButton(usize),
    ContextEditButton(usize),
    ContextDuplicateButton(usize),
    ContextDeleteButton(usize),
    ContextAddButton,
    ContextToggleMuteOutputs,
    ContextToggleMuteInputs,
    ContextToggleMinimalist,
    ContextToggleTheme,
    ContextToggleLabels,
}

/// Shared wakeup & Win32 bridge so tray/menu callbacks can act on the `HWND` directly
/// even when the window is hidden.
pub struct TrayWakeupBridge {
    pub hwnd: AtomicIsize,
    pub always_on_top: AtomicBool,
    pub quit_requested: AtomicBool,
    pub context_target_btn: AtomicIsize,
    pub egui_ctx: Mutex<Option<egui::Context>>,
}

impl TrayWakeupBridge {
    pub fn new(initial_always_on_top: bool) -> Arc<Self> {
        Arc::new(Self {
            hwnd: AtomicIsize::new(0),
            always_on_top: AtomicBool::new(initial_always_on_top),
            quit_requested: AtomicBool::new(false),
            context_target_btn: AtomicIsize::new(-1),
            egui_ctx: Mutex::new(None),
        })
    }

    pub fn set_hwnd(&self, hwnd: isize) {
        self.hwnd.store(hwnd, Ordering::SeqCst);
    }

    pub fn set_egui_ctx(&self, ctx: egui::Context) {
        if let Ok(mut g) = self.egui_ctx.lock() {
            *g = Some(ctx);
        }
    }

    pub fn wake(&self) {
        let hwnd = self.hwnd.load(Ordering::SeqCst);
        if hwnd != 0 {
            win32_window::wake_window_event_loop(hwnd);
        }
        if let Ok(g) = self.egui_ctx.lock() {
            if let Some(ctx) = g.as_ref() {
                ctx.request_repaint();
            }
        }
    }
}

/// Holds references to dynamic popup menu items so `muda` keeps their IDs valid.
struct ActivePopupHolders {
    _menu: Menu,
    _items: Vec<MenuItem>,
    _checks: Vec<CheckMenuItem>,
    _seps: Vec<PredefinedMenuItem>,
}

pub struct TrayManager {
    _tray_icon: TrayIcon,
    _menu: Menu,
    _show_hide_item: MenuItem,
    always_on_top_item: CheckMenuItem,
    _open_config_item: MenuItem,
    _quit_item: MenuItem,
    active_popup: Option<ActivePopupHolders>,
    rx: Receiver<TrayCommand>,
    bridge: Arc<TrayWakeupBridge>,
}

impl TrayManager {
    pub fn new(initial_always_on_top: bool) -> Result<Self, String> {
        let icon = load_embedded_tray_icon()?;

        let menu = Menu::new();
        let show_hide_item = MenuItem::with_id(
            MenuId::new(MENU_SHOW_HIDE),
            "Show / Hide Buttons",
            true,
            None,
        );
        let always_on_top_item = CheckMenuItem::with_id(
            MenuId::new(MENU_ALWAYS_ON_TOP),
            "Always on Top",
            true,
            initial_always_on_top,
            None,
        );
        let open_config_item = MenuItem::with_id(
            MenuId::new(MENU_OPEN_CONFIG),
            "Open Configuration / Edit Mode",
            true,
            None,
        );
        let separator = PredefinedMenuItem::separator();
        let quit_item = MenuItem::with_id(MenuId::new(MENU_QUIT), "Quit Lyre", true, None);

        menu.append_items(&[
            &show_hide_item,
            &always_on_top_item,
            &open_config_item,
            &separator,
            &quit_item,
        ])
        .map_err(|e| format!("Failed to build tray context menu: {}", e))?;

        let tray_icon = TrayIconBuilder::new()
            .with_tooltip("Lyre — Single-Click Audio Switcher")
            .with_icon(icon)
            .with_menu(Box::new(menu.clone()))
            .with_menu_on_left_click(false)
            .build()
            .map_err(|e| format!("Failed to create system tray icon: {}", e))?;

        let (tx, rx) = mpsc::channel();
        let bridge = TrayWakeupBridge::new(initial_always_on_top);

        // 1. Left-clicking the tray icon immediately toggles Show / Hide via Win32 HWND
        let tx_tray = tx.clone();
        let bridge_tray = bridge.clone();
        TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let hwnd = bridge_tray.hwnd.load(Ordering::SeqCst);
                let currently_vis = win32_window::is_window_visible_win32(hwnd);
                let target_vis = !currently_vis;
                win32_window::set_window_visible_win32(hwnd, target_vis);
                win32_window::apply_tool_window_style(hwnd);
                let _ = tx_tray.send(TrayCommand::SetVisibility(target_vis));
                bridge_tray.wake();
            }
        }));

        // 2. Context menu item click handler (handles both Tray Menu and Window Right-Click Menu)
        let tx_menu = tx;
        let bridge_menu = bridge.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let hwnd = bridge_menu.hwnd.load(Ordering::SeqCst);
            let target_btn = bridge_menu.context_target_btn.load(Ordering::SeqCst);

            match event.id().0.as_str() {
                MENU_SHOW_HIDE => {
                    let currently_vis = win32_window::is_window_visible_win32(hwnd);
                    let target_vis = !currently_vis;
                    win32_window::set_window_visible_win32(hwnd, target_vis);
                    win32_window::apply_tool_window_style(hwnd);
                    let _ = tx_menu.send(TrayCommand::SetVisibility(target_vis));
                    bridge_menu.wake();
                }
                MENU_ALWAYS_ON_TOP => {
                    let prev = bridge_menu.always_on_top.load(Ordering::SeqCst);
                    let next = !prev;
                    bridge_menu.always_on_top.store(next, Ordering::SeqCst);
                    win32_window::set_always_on_top_win32(hwnd, next);
                    let _ = tx_menu.send(TrayCommand::SetAlwaysOnTop(next));
                    bridge_menu.wake();
                }
                MENU_OPEN_CONFIG => {
                    win32_window::set_window_visible_win32(hwnd, true);
                    win32_window::apply_tool_window_style(hwnd);
                    let _ = tx_menu.send(TrayCommand::OpenConfigMode);
                    bridge_menu.wake();
                }
                MENU_QUIT => {
                    bridge_menu.quit_requested.store(true, Ordering::SeqCst);
                    let _ = tx_menu.send(TrayCommand::Quit);
                    win32_window::post_close_win32(hwnd);
                    bridge_menu.wake();
                    std::thread::spawn(|| {
                        std::thread::sleep(std::time::Duration::from_millis(120));
                        std::process::exit(0);
                    });
                }
                CTX_SWITCH_BTN if target_btn >= 0 => {
                    let _ = tx_menu.send(TrayCommand::ContextSwitchButton(target_btn as usize));
                    bridge_menu.wake();
                }
                CTX_EDIT_BTN if target_btn >= 0 => {
                    let _ = tx_menu.send(TrayCommand::ContextEditButton(target_btn as usize));
                    bridge_menu.wake();
                }
                CTX_DUPLICATE_BTN if target_btn >= 0 => {
                    let _ =
                        tx_menu.send(TrayCommand::ContextDuplicateButton(target_btn as usize));
                    bridge_menu.wake();
                }
                CTX_DELETE_BTN if target_btn >= 0 => {
                    let _ = tx_menu.send(TrayCommand::ContextDeleteButton(target_btn as usize));
                    bridge_menu.wake();
                }
                CTX_ADD_BTN => {
                    let _ = tx_menu.send(TrayCommand::ContextAddButton);
                    bridge_menu.wake();
                }
                CTX_MUTE_OUTPUTS => {
                    let _ = tx_menu.send(TrayCommand::ContextToggleMuteOutputs);
                    bridge_menu.wake();
                }
                CTX_MUTE_INPUTS => {
                    let _ = tx_menu.send(TrayCommand::ContextToggleMuteInputs);
                    bridge_menu.wake();
                }
                CTX_TOGGLE_MINIMALIST => {
                    let _ = tx_menu.send(TrayCommand::ContextToggleMinimalist);
                    bridge_menu.wake();
                }
                CTX_TOGGLE_THEME => {
                    let _ = tx_menu.send(TrayCommand::ContextToggleTheme);
                    bridge_menu.wake();
                }
                CTX_TOGGLE_LABELS => {
                    let _ = tx_menu.send(TrayCommand::ContextToggleLabels);
                    bridge_menu.wake();
                }
                _ => {}
            }
        }));

        Ok(Self {
            _tray_icon: tray_icon,
            _menu: menu,
            _show_hide_item: show_hide_item,
            always_on_top_item,
            _open_config_item: open_config_item,
            _quit_item: quit_item,
            active_popup: None,
            rx,
            bridge,
        })
    }

    pub fn bind_window_and_context(&self, hwnd: isize, ctx: egui::Context) {
        self.bridge.set_hwnd(hwnd);
        self.bridge.set_egui_ctx(ctx);
    }

    pub fn set_always_on_top_checked(&self, checked: bool) {
        self.bridge.always_on_top.store(checked, Ordering::SeqCst);
        self.always_on_top_item.set_checked(checked);
    }

    pub fn is_quit_requested(&self) -> bool {
        self.bridge.quit_requested.load(Ordering::SeqCst)
    }

    pub fn poll_command(&self) -> Option<TrayCommand> {
        self.rx.try_recv().ok()
    }

    /// Displays a native Win32 right-click context menu at the cursor position for `hwnd`.
    /// Because it uses OS `TrackPopupMenu`, it can extend freely outside even a tiny frameless window.
    #[allow(clippy::too_many_arguments)]
    pub fn show_window_context_menu(
        &mut self,
        hwnd: isize,
        button_target: Option<(usize, &str)>,
        outputs_muted: bool,
        inputs_muted: bool,
        minimalist_mode: bool,
        dark_mode: bool,
        labels_shown: bool,
        always_on_top: bool,
    ) {
        if hwnd == 0 {
            return;
        }

        let popup = Menu::new();
        let mut items = Vec::new();
        let mut checks = Vec::new();
        let mut seps = Vec::new();

        if let Some((idx, title)) = button_target {
            self.bridge
                .context_target_btn
                .store(idx as isize, Ordering::SeqCst);

            let switch_item = MenuItem::with_id(
                MenuId::new(CTX_SWITCH_BTN),
                format!("Switch to \"{}\"", title),
                true,
                None,
            );
            let edit_item = MenuItem::with_id(
                MenuId::new(CTX_EDIT_BTN),
                format!("Customize \"{}\"…", title),
                true,
                None,
            );
            let dup_item = MenuItem::with_id(
                MenuId::new(CTX_DUPLICATE_BTN),
                "Duplicate Button",
                true,
                None,
            );
            let del_item =
                MenuItem::with_id(MenuId::new(CTX_DELETE_BTN), "Delete Button", true, None);
            let sep1 = PredefinedMenuItem::separator();

            let _ = popup.append_items(&[&switch_item, &edit_item, &dup_item, &del_item, &sep1]);
            items.extend([switch_item, edit_item, dup_item, del_item]);
            seps.push(sep1);
        } else {
            self.bridge.context_target_btn.store(-1, Ordering::SeqCst);
        }

        let add_item = MenuItem::with_id(
            MenuId::new(CTX_ADD_BTN),
            "Add New Audio Button…",
            true,
            None,
        );
        let sep_audio = PredefinedMenuItem::separator();
        let mute_out_check = CheckMenuItem::with_id(
            MenuId::new(CTX_MUTE_OUTPUTS),
            "Mute All Outputs (Speakers / Headphones)",
            true,
            outputs_muted,
            None,
        );
        let mute_in_check = CheckMenuItem::with_id(
            MenuId::new(CTX_MUTE_INPUTS),
            "Mute All Microphones",
            true,
            inputs_muted,
            None,
        );
        let sep_view = PredefinedMenuItem::separator();
        let min_check = CheckMenuItem::with_id(
            MenuId::new(CTX_TOGGLE_MINIMALIST),
            "Minimalist Frameless Mode",
            true,
            minimalist_mode,
            None,
        );
        let theme_check = CheckMenuItem::with_id(
            MenuId::new(CTX_TOGGLE_THEME),
            "Dark Mode",
            true,
            dark_mode,
            None,
        );
        let labels_check = CheckMenuItem::with_id(
            MenuId::new(CTX_TOGGLE_LABELS),
            "Show Text Labels",
            true,
            labels_shown,
            None,
        );
        let top_check = CheckMenuItem::with_id(
            MenuId::new(MENU_ALWAYS_ON_TOP),
            "Always on Top",
            true,
            always_on_top,
            None,
        );
        let sep_app = PredefinedMenuItem::separator();
        let hide_item = MenuItem::with_id(
            MenuId::new(MENU_SHOW_HIDE),
            "Hide Window to System Tray",
            true,
            None,
        );
        let quit_item = MenuItem::with_id(MenuId::new(MENU_QUIT), "Quit Lyre", true, None);

        let _ = popup.append_items(&[
            &add_item,
            &sep_audio,
            &mute_out_check,
            &mute_in_check,
            &sep_view,
            &min_check,
            &theme_check,
            &labels_check,
            &top_check,
            &sep_app,
            &hide_item,
            &quit_item,
        ]);

        items.extend([add_item, hide_item, quit_item]);
        checks.extend([
            mute_out_check,
            mute_in_check,
            min_check,
            theme_check,
            labels_check,
            top_check,
        ]);
        seps.extend([sep_audio, sep_view, sep_app]);

        unsafe {
            let _ = popup.show_context_menu_for_hwnd(hwnd, None);
        }

        self.active_popup = Some(ActivePopupHolders {
            _menu: popup,
            _items: items,
            _checks: checks,
            _seps: seps,
        });
    }
}

/// Crops any outer transparent margin around an RGBA image and centers it inside a square
/// transparent canvas with a subtle 2px breathing margin so the icon fills the taskbar/tray slot.
fn crop_transparent_logo_to_square(img: &image::DynamicImage) -> image::RgbaImage {
    let src = img.to_rgba8();
    let (w, h) = src.dimensions();
    let mut min_x = w;
    let mut min_y = h;
    let mut max_x = 0u32;
    let mut max_y = 0u32;

    for y in 0..h {
        for x in 0..w {
            if src.get_pixel(x, y)[3] > 10 {
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
    }

    if min_x > max_x || min_y > max_y {
        return src;
    }

    let crop_w = max_x - min_x + 1;
    let crop_h = max_y - min_y + 1;
    let cropped = image::imageops::crop_imm(&src, min_x, min_y, crop_w, crop_h).to_image();

    let side = crop_w.max(crop_h);
    let pad = (side as f32 * 0.03).round() as u32;
    let canvas_side = side + pad * 2;
    let mut square = image::RgbaImage::from_pixel(canvas_side, canvas_side, image::Rgba([0, 0, 0, 0]));
    let offset_x = ((canvas_side - crop_w) / 2) as i64;
    let offset_y = ((canvas_side - crop_h) / 2) as i64;
    image::imageops::overlay(&mut square, &cropped, offset_x, offset_y);
    square
}

/// Loads `logo_bg.png` (transparent background, embedded at compile time) as a 64x64 RGBA `tray_icon::Icon`
/// for the Windows Taskbar / System Tray notification area.
pub fn load_embedded_tray_icon() -> Result<Icon, String> {
    let bytes = include_bytes!("../../logo_bg.png");
    let img = image::load_from_memory(bytes)
        .map_err(|e| format!("Failed to decode embedded logo_bg.png: {}", e))?;
    let square = crop_transparent_logo_to_square(&img);
    let resized = image::imageops::resize(&square, 64, 64, image::imageops::FilterType::Lanczos3);
    let (w, h) = resized.dimensions();
    Icon::from_rgba(resized.into_raw(), w, h)
        .map_err(|e| format!("Failed to construct tray Icon: {}", e))
}

/// Loads `logo.png` (logo with background, embedded at compile time) as a 64x64 RGBA `egui::IconData`
/// for the window title bar icon.
pub fn load_embedded_egui_icon() -> Option<egui::IconData> {
    let bytes = include_bytes!("../../logo.png");
    let img = image::load_from_memory(bytes).ok()?;
    let resized = img.resize_exact(64, 64, image::imageops::FilterType::Lanczos3);
    let rgba = resized.to_rgba8();
    let (width, height) = rgba.dimensions();
    Some(egui::IconData {
        rgba: rgba.into_raw(),
        width,
        height,
    })
}
