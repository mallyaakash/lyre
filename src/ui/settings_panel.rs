use std::time::{Duration, Instant};

use egui::{
    Align2, CentralPanel, Color32, Context, CursorIcon, FontFamily, FontId, Frame as EguiFrame,
    Margin, Pos2, Rect, RichText, Rounding, ScrollArea, Sense, Stroke, Ui, Vec2, ViewportBuilder,
    ViewportClass, ViewportCommand, ViewportId,
};

use crate::audio::{AudioEngine, EndpointDirection};
use crate::config::{
    format_hex_color, parse_hex_color, shorten_device_title, AppConfig, IconSource, PresetIcon,
    ThemeMode, DEFAULT_COLOR_PALETTE,
};
use crate::tray::win32_window::{
    compute_same_monitor_cursor_menu_pos, compute_same_monitor_dialog_pos,
};
use crate::ui::grid::create_new_button_from_audio;
use crate::ui::icons::IconCache;
use crate::ui::theme::apply_theme;

/// Tracks the state of the custom **Linear / Raycast Matte Obsidian** right-click context window.
#[derive(Debug, Clone)]
pub struct ContextMenuState {
    /// If the user right-clicked on a specific button card, holds its index.
    pub target_button_idx: Option<usize>,
    /// Logical screen coordinates `[x, y]` where the context window spawns.
    pub spawn_pos: [f32; 2],
    /// Timestamp when the context window was opened (used for focus-loss dismissal).
    pub opened_at: Instant,
    /// Whether the context window has observed OS focus yet.
    pub had_focus: bool,
    /// Whether the context window needs initial positioning on this open cycle.
    pub just_opened: bool,
}

/// Action triggered by interacting with the custom right-click context window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextMenuAction {
    None,
    SwitchButton(usize),
    EditButton(usize),
    DuplicateButton(usize),
    DeleteButton(usize),
    AddButton,
    ToggleMuteOutputs,
    ToggleMuteInputs,
    ToggleMinimalist,
    ToggleTheme,
    ToggleLabels,
    ToggleAlwaysOnTop,
    HideToTray,
    QuitApp,
}

#[derive(Debug, Default)]
pub struct EditorState {
    /// Index of the button currently being edited in the standalone dialog window, if any.
    pub selected_button_idx: Option<usize>,
    /// Status banner message (e.g., last switched device or error).
    pub status_message: Option<String>,
    /// Cached initial position on the same monitor as the main window when the dialog opens.
    pub dialog_spawn_pos: Option<[f32; 2]>,
    /// Tracks whether the external viewport was already open on the previous frame.
    pub was_open: bool,
    /// Active custom Linear / Raycast Matte Obsidian right-click context window state.
    pub context_menu: Option<ContextMenuState>,
}

impl EditorState {
    /// Opens the custom **Linear / Raycast Matte Obsidian** right-click context window
    /// right at the cursor on the same monitor as `main_hwnd`.
    pub fn open_context_menu(
        &mut self,
        main_hwnd: Option<isize>,
        target_button_idx: Option<usize>,
        pixels_per_point: f32,
    ) {
        let menu_w = 292.0;
        let menu_h = if target_button_idx.is_some() {
            504.0
        } else {
            356.0
        };
        let spawn_pos = compute_same_monitor_cursor_menu_pos(
            main_hwnd.unwrap_or(0),
            menu_w,
            menu_h,
            pixels_per_point,
        )
        .unwrap_or([240.0, 240.0]);

        self.context_menu = Some(ContextMenuState {
            target_button_idx,
            spawn_pos,
            opened_at: Instant::now(),
            had_focus: false,
            just_opened: true,
        });
    }
}

/// Vector icons for toolbar and dialog action pill buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolbarIcon {
    ThemeMoon,
    ThemeSun,
    TextLabel,
    Speaker { muted: bool },
    Microphone { muted: bool },
    ColumnsGrid,
    Plus,
    Minimalist,
    Pin { pinned: bool },
    ArrowLeft,
    ArrowRight,
    Trash,
    Check,
}

/// Draws a crisp anti-aliased 14x14 vector icon inside a pill button or toolbar header.
pub fn draw_toolbar_icon(
    painter: &egui::Painter,
    center: Pos2,
    icon: ToolbarIcon,
    color: Color32,
) {
    let stroke = Stroke::new(1.55, color);
    match icon {
        ToolbarIcon::ThemeMoon => {
            painter.circle_stroke(center, 4.8, stroke);
            painter.line_segment(
                [Pos2::new(center.x + 1.0, center.y - 4.2), Pos2::new(center.x + 1.0, center.y + 4.2)],
                Stroke::new(1.4, color),
            );
        }
        ToolbarIcon::ThemeSun => {
            painter.circle_stroke(center, 3.2, stroke);
            for i in 0..8 {
                let angle = (i as f32) * std::f32::consts::TAU / 8.0;
                let p1 = Pos2::new(center.x + angle.cos() * 4.6, center.y + angle.sin() * 4.6);
                let p2 = Pos2::new(center.x + angle.cos() * 6.2, center.y + angle.sin() * 6.2);
                painter.line_segment([p1, p2], Stroke::new(1.3, color));
            }
        }
        ToolbarIcon::TextLabel => {
            // Crisp 'T' inside a subtle bracket
            painter.line_segment(
                [Pos2::new(center.x - 4.5, center.y - 4.2), Pos2::new(center.x + 4.5, center.y - 4.2)],
                Stroke::new(1.7, color),
            );
            painter.line_segment(
                [Pos2::new(center.x, center.y - 4.2), Pos2::new(center.x, center.y + 4.5)],
                Stroke::new(1.7, color),
            );
            painter.line_segment(
                [Pos2::new(center.x - 2.4, center.y + 4.5), Pos2::new(center.x + 2.4, center.y + 4.5)],
                Stroke::new(1.4, color),
            );
        }
        ToolbarIcon::Speaker { muted } => {
            // Speaker body + cone
            let box_rect = Rect::from_min_max(
                Pos2::new(center.x - 5.8, center.y - 2.2),
                Pos2::new(center.x - 3.2, center.y + 2.2),
            );
            painter.rect_filled(box_rect, Rounding::same(1.0), color);
            painter.line_segment(
                [Pos2::new(center.x - 3.2, center.y - 2.2), Pos2::new(center.x + 0.5, center.y - 5.0)],
                stroke,
            );
            painter.line_segment(
                [Pos2::new(center.x + 0.5, center.y - 5.0), Pos2::new(center.x + 0.5, center.y + 5.0)],
                stroke,
            );
            painter.line_segment(
                [Pos2::new(center.x + 0.5, center.y + 5.0), Pos2::new(center.x - 3.2, center.y + 2.2)],
                stroke,
            );
            if muted {
                let red = Color32::from_rgb(248, 113, 113);
                let s = Stroke::new(1.7, red);
                painter.line_segment(
                    [Pos2::new(center.x + 2.2, center.y - 2.8), Pos2::new(center.x + 6.4, center.y + 2.8)],
                    s,
                );
                painter.line_segment(
                    [Pos2::new(center.x + 6.4, center.y - 2.8), Pos2::new(center.x + 2.2, center.y + 2.8)],
                    s,
                );
            } else {
                painter.line_segment(
                    [Pos2::new(center.x + 2.8, center.y - 2.4), Pos2::new(center.x + 3.8, center.y)],
                    stroke,
                );
                painter.line_segment(
                    [Pos2::new(center.x + 3.8, center.y), Pos2::new(center.x + 2.8, center.y + 2.4)],
                    stroke,
                );
                painter.line_segment(
                    [Pos2::new(center.x + 4.8, center.y - 4.2), Pos2::new(center.x + 6.2, center.y)],
                    stroke,
                );
                painter.line_segment(
                    [Pos2::new(center.x + 6.2, center.y), Pos2::new(center.x + 4.8, center.y + 4.2)],
                    stroke,
                );
            }
        }
        ToolbarIcon::Microphone { muted } => {
            // Studio microphone capsule + U-stand + base
            let cap_rect = Rect::from_center_size(
                Pos2::new(center.x, center.y - 1.8),
                Vec2::new(4.6, 6.6),
            );
            painter.rect_stroke(cap_rect, Rounding::same(2.3), stroke);
            // U-cradle
            painter.line_segment(
                [Pos2::new(center.x - 4.2, center.y - 1.2), Pos2::new(center.x - 4.2, center.y + 2.0)],
                stroke,
            );
            painter.line_segment(
                [Pos2::new(center.x + 4.2, center.y - 1.2), Pos2::new(center.x + 4.2, center.y + 2.0)],
                stroke,
            );
            painter.line_segment(
                [Pos2::new(center.x - 4.2, center.y + 2.0), Pos2::new(center.x + 4.2, center.y + 2.0)],
                stroke,
            );
            // Stand stem + base
            painter.line_segment(
                [Pos2::new(center.x, center.y + 2.0), Pos2::new(center.x, center.y + 5.2)],
                stroke,
            );
            painter.line_segment(
                [Pos2::new(center.x - 2.8, center.y + 5.2), Pos2::new(center.x + 2.8, center.y + 5.2)],
                stroke,
            );
            if muted {
                let red = Color32::from_rgb(248, 113, 113);
                painter.line_segment(
                    [Pos2::new(center.x - 5.4, center.y - 5.2), Pos2::new(center.x + 5.4, center.y + 5.2)],
                    Stroke::new(1.85, red),
                );
            }
        }
        ToolbarIcon::ColumnsGrid => {
            let r = 1.0;
            for (dx, dy) in [(-2.8, -2.8), (2.8, -2.8), (-2.8, 2.8), (2.8, 2.8)] {
                let cell = Rect::from_center_size(
                    Pos2::new(center.x + dx, center.y + dy),
                    Vec2::splat(4.0),
                );
                painter.rect_stroke(cell, Rounding::same(r), Stroke::new(1.3, color));
            }
        }
        ToolbarIcon::Plus => {
            painter.line_segment(
                [Pos2::new(center.x - 4.5, center.y), Pos2::new(center.x + 4.5, center.y)],
                Stroke::new(1.8, color),
            );
            painter.line_segment(
                [Pos2::new(center.x, center.y - 4.5), Pos2::new(center.x, center.y + 4.5)],
                Stroke::new(1.8, color),
            );
        }
        ToolbarIcon::Minimalist => {
            // 4 crisp corner brackets
            let d = 4.6;
            let l = 2.4;
            // Top-left
            painter.line_segment([Pos2::new(center.x - d, center.y - d), Pos2::new(center.x - d + l, center.y - d)], stroke);
            painter.line_segment([Pos2::new(center.x - d, center.y - d), Pos2::new(center.x - d, center.y - d + l)], stroke);
            // Top-right
            painter.line_segment([Pos2::new(center.x + d, center.y - d), Pos2::new(center.x + d - l, center.y - d)], stroke);
            painter.line_segment([Pos2::new(center.x + d, center.y - d), Pos2::new(center.x + d, center.y - d + l)], stroke);
            // Bottom-left
            painter.line_segment([Pos2::new(center.x - d, center.y + d), Pos2::new(center.x - d + l, center.y + d)], stroke);
            painter.line_segment([Pos2::new(center.x - d, center.y + d), Pos2::new(center.x - d, center.y + d - l)], stroke);
            // Bottom-right
            painter.line_segment([Pos2::new(center.x + d, center.y + d), Pos2::new(center.x + d - l, center.y + d)], stroke);
            painter.line_segment([Pos2::new(center.x + d, center.y + d), Pos2::new(center.x + d, center.y + d - l)], stroke);
        }
        ToolbarIcon::Pin { pinned } => {
            let pin_col = if pinned {
                Color32::from_rgb(59, 130, 246)
            } else {
                color
            };
            let s = Stroke::new(1.55, pin_col);
            // Top pushpin cap bar
            painter.line_segment(
                [Pos2::new(center.x - 3.2, center.y - 5.0), Pos2::new(center.x + 3.2, center.y - 5.0)],
                Stroke::new(1.75, pin_col),
            );
            // Pin upper cylinder body
            let body_rect = Rect::from_min_max(
                Pos2::new(center.x - 2.1, center.y - 5.0),
                Pos2::new(center.x + 2.1, center.y - 0.6),
            );
            if pinned {
                painter.rect_filled(body_rect, Rounding::same(0.8), pin_col);
            } else {
                painter.rect_stroke(body_rect, Rounding::same(0.8), s);
            }
            // Lower flange bar
            painter.line_segment(
                [Pos2::new(center.x - 4.4, center.y - 0.6), Pos2::new(center.x + 4.4, center.y - 0.6)],
                Stroke::new(1.75, pin_col),
            );
            // Sharp needle stem pointing down
            painter.line_segment(
                [Pos2::new(center.x, center.y - 0.6), Pos2::new(center.x, center.y + 5.2)],
                Stroke::new(1.65, pin_col),
            );
        }
        ToolbarIcon::ArrowLeft => {
            let s = Stroke::new(1.75, color);
            painter.line_segment(
                [Pos2::new(center.x + 4.8, center.y), Pos2::new(center.x - 4.5, center.y)],
                s,
            );
            painter.line_segment(
                [Pos2::new(center.x - 0.8, center.y - 3.8), Pos2::new(center.x - 4.5, center.y)],
                s,
            );
            painter.line_segment(
                [Pos2::new(center.x - 0.8, center.y + 3.8), Pos2::new(center.x - 4.5, center.y)],
                s,
            );
        }
        ToolbarIcon::ArrowRight => {
            let s = Stroke::new(1.75, color);
            painter.line_segment(
                [Pos2::new(center.x - 4.8, center.y), Pos2::new(center.x + 4.5, center.y)],
                s,
            );
            painter.line_segment(
                [Pos2::new(center.x + 0.8, center.y - 3.8), Pos2::new(center.x + 4.5, center.y)],
                s,
            );
            painter.line_segment(
                [Pos2::new(center.x + 0.8, center.y + 3.8), Pos2::new(center.x + 4.5, center.y)],
                s,
            );
        }
        ToolbarIcon::Trash => {
            painter.line_segment(
                [Pos2::new(center.x - 4.6, center.y - 3.2), Pos2::new(center.x + 4.6, center.y - 3.2)],
                stroke,
            );
            painter.line_segment(
                [Pos2::new(center.x - 1.8, center.y - 4.8), Pos2::new(center.x + 1.8, center.y - 4.8)],
                stroke,
            );
            let bin = Rect::from_min_max(
                Pos2::new(center.x - 3.4, center.y - 3.0),
                Pos2::new(center.x + 3.4, center.y + 4.8),
            );
            painter.rect_stroke(bin, Rounding::same(1.2), stroke);
        }
        ToolbarIcon::Check => {
            let s = Stroke::new(1.85, color);
            painter.line_segment(
                [Pos2::new(center.x - 4.2, center.y + 0.2), Pos2::new(center.x - 1.2, center.y + 3.4)],
                s,
            );
            painter.line_segment(
                [Pos2::new(center.x - 1.2, center.y + 3.4), Pos2::new(center.x + 4.6, center.y - 3.2)],
                s,
            );
        }
    }
}

/// Custom-painted uniform matte pill button (**Linear / Raycast Matte Obsidian** style)
/// with optional vector icon, crimson danger/mute state, and enabled/disabled support.
#[allow(clippy::too_many_arguments)]
pub fn pill_button_with_icon(
    ui: &mut Ui,
    icon: Option<ToolbarIcon>,
    text: &str,
    active_state: bool,
    accent_highlight: bool,
    danger_state: bool,
    enabled: bool,
    theme_mode: ThemeMode,
) -> egui::Response {
    let font_id = FontId::new(12.2, FontFamily::Proportional);
    let is_icon_only = text.is_empty() && icon.is_some();
    let desired_size = if is_icon_only {
        Vec2::new(28.0, 26.0)
    } else {
        let galley = ui
            .painter()
            .layout_no_wrap(text.to_string(), font_id.clone(), Color32::WHITE);
        let icon_pad = if icon.is_some() { 18.0 } else { 0.0 };
        Vec2::new(galley.size().x + 18.0 + icon_pad, 26.0)
    };
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, response) = ui.allocate_exact_size(desired_size, sense);

    if enabled && response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }

    let hovered = enabled && response.hovered();

    let (mut bg_fill, mut border_col, mut text_col) = match theme_mode {
        ThemeMode::Dark => {
            if danger_state {
                let bg = if hovered {
                    Color32::from_rgba_unmultiplied(239, 68, 68, 42)
                } else {
                    Color32::from_rgba_unmultiplied(239, 68, 68, 22)
                };
                let border = if hovered || active_state {
                    Color32::from_rgb(239, 68, 68)
                } else {
                    Color32::from_rgba_unmultiplied(239, 68, 68, 120)
                };
                (bg, border, Color32::from_rgb(252, 165, 165))
            } else if accent_highlight {
                let bg = if hovered {
                    Color32::from_rgb(44, 48, 59)
                } else {
                    Color32::from_rgb(33, 36, 45)
                };
                (
                    bg,
                    Color32::from_rgb(76, 82, 98),
                    Color32::from_rgb(248, 249, 252),
                )
            } else if active_state {
                let bg = if hovered {
                    Color32::from_rgb(38, 42, 51)
                } else {
                    Color32::from_rgb(28, 31, 38)
                };
                (
                    bg,
                    Color32::from_rgb(62, 67, 82),
                    Color32::from_rgb(240, 242, 248),
                )
            } else {
                let bg = if hovered {
                    Color32::from_rgb(29, 32, 39)
                } else {
                    Color32::from_rgb(21, 23, 28)
                };
                (
                    bg,
                    Color32::from_rgb(40, 44, 54),
                    Color32::from_rgb(190, 195, 208),
                )
            }
        }
        ThemeMode::Light => {
            if danger_state {
                (
                    Color32::from_rgba_unmultiplied(220, 38, 38, 22),
                    Color32::from_rgb(220, 38, 38),
                    Color32::from_rgb(185, 28, 28),
                )
            } else if accent_highlight || active_state {
                let bg = if hovered {
                    Color32::from_rgb(220, 224, 234)
                } else {
                    Color32::from_rgb(230, 233, 242)
                };
                (
                    bg,
                    Color32::from_rgb(184, 190, 204),
                    Color32::from_rgb(20, 22, 28),
                )
            } else {
                let bg = if hovered {
                    Color32::from_rgb(234, 236, 243)
                } else {
                    Color32::from_rgb(244, 245, 249)
                };
                (
                    bg,
                    Color32::from_rgb(210, 214, 224),
                    Color32::from_rgb(60, 64, 74),
                )
            }
        }
    };

    if !enabled {
        bg_fill = Color32::from_rgba_unmultiplied(bg_fill.r(), bg_fill.g(), bg_fill.b(), 90);
        border_col =
            Color32::from_rgba_unmultiplied(border_col.r(), border_col.g(), border_col.b(), 80);
        text_col = Color32::from_rgba_unmultiplied(text_col.r(), text_col.g(), text_col.b(), 90);
    }

    let rounding = Rounding::same(6.5);
    ui.painter().rect_filled(rect, rounding, bg_fill);
    ui.painter()
        .rect_stroke(rect, rounding, Stroke::new(1.0, border_col));

    if let Some(ic) = icon {
        if is_icon_only {
            draw_toolbar_icon(ui.painter(), rect.center(), ic, text_col);
        } else {
            let icon_center = Pos2::new(rect.left() + 13.5, rect.center().y);
            draw_toolbar_icon(ui.painter(), icon_center, ic, text_col);
            ui.painter().text(
                Pos2::new(rect.left() + 24.5, rect.center().y),
                Align2::LEFT_CENTER,
                text,
                font_id,
                text_col,
            );
        }
    } else {
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            text,
            font_id,
            text_col,
        );
    }

    response
}

/// Custom-painted uniform matte pill button (**Linear / Raycast Matte Obsidian** style,
/// zero two-tone bands, crisp 1px hairline border).
pub fn pill_button(
    ui: &mut Ui,
    text: &str,
    active_state: bool,
    accent_highlight: bool,
    theme_mode: ThemeMode,
) -> egui::Response {
    pill_button_with_icon(
        ui,
        None,
        text,
        active_state,
        accent_highlight,
        false,
        true,
        theme_mode,
    )
}

/// Custom-painted Linear / iOS style pill toggle switch (`true` / `false`).
fn draw_toggle_switch(
    ui: &mut Ui,
    value: &mut bool,
    label: &str,
    accent: Color32,
    theme_mode: ThemeMode,
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        let switch_size = Vec2::new(34.0, 19.0);
        let (rect, resp) = ui.allocate_exact_size(switch_size, Sense::click());
        if resp.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if resp.clicked() {
            *value = !*value;
            changed = true;
        }

        let rounding = Rounding::same(9.5);
        let (track_bg, track_border) = if *value {
            (accent, accent)
        } else {
            match theme_mode {
                ThemeMode::Dark => (
                    Color32::from_rgb(30, 33, 41),
                    Color32::from_rgb(52, 56, 68),
                ),
                ThemeMode::Light => (
                    Color32::from_rgb(214, 218, 228),
                    Color32::from_rgb(186, 192, 204),
                ),
            }
        };

        ui.painter().rect_filled(rect, rounding, track_bg);
        ui.painter()
            .rect_stroke(rect, rounding, Stroke::new(1.0, track_border));

        let knob_r = 6.8;
        let knob_x = if *value {
            rect.right() - 9.5
        } else {
            rect.left() + 9.5
        };
        ui.painter().circle_filled(
            Pos2::new(knob_x, rect.center().y),
            knob_r,
            Color32::WHITE,
        );

        let label_resp = ui.add(
            egui::Label::new(RichText::new(label).size(12.5)).sense(Sense::click()),
        );
        if label_resp.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if label_resp.clicked() {
            *value = !*value;
            changed = true;
        }
    });
    changed
}

/// Renders the top Edit/Configuration toolbar when Lyre is in Configuration / Edit Mode:
/// - **Left Group:** Theme (with Moon/Sun icon), Labels (with T icon), Mute Outputs (with Speaker icon), Mute Mics (with Microphone icon)
/// - **Middle Group:** Columns selector (`[⊞ Columns  −  N  + ]`)
/// - **Right Group:** `+ Add Button` & `Minimalist Mode`
pub fn render_top_config_bar(
    ui: &mut Ui,
    config: &mut AppConfig,
    audio: &mut AudioEngine,
    editor: &mut EditorState,
    enter_minimalist_requested: &mut bool,
) -> bool {
    let mut changed = false;
    let theme = config.window.theme_mode;
    let muted_text = match theme {
        ThemeMode::Dark => Color32::from_rgb(148, 154, 168),
        ThemeMode::Light => Color32::from_rgb(90, 95, 108),
    };

    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(5.0, 4.0);

        // ── 1. LEFT GROUP: Pin on Top (icon-only), Theme, Labels, Mute Outputs, Mute Inputs ──
        let is_pinned = config.window.always_on_top;
        let pin_tooltip = if is_pinned {
            "Always on Top: On (Click to unpin window)"
        } else {
            "Always on Top: Off (Click to pin window above other apps)"
        };
        if pill_button_with_icon(
            ui,
            Some(ToolbarIcon::Pin { pinned: is_pinned }),
            "",
            is_pinned,
            is_pinned,
            false,
            true,
            theme,
        )
        .on_hover_text(pin_tooltip)
        .clicked()
        {
            config.window.always_on_top = !config.window.always_on_top;
            changed = true;
        }

        let (theme_icon, theme_label) = match config.window.theme_mode {
            ThemeMode::Dark => (ToolbarIcon::ThemeMoon, "Dark"),
            ThemeMode::Light => (ToolbarIcon::ThemeSun, "Light"),
        };
        if pill_button_with_icon(ui, Some(theme_icon), theme_label, true, false, false, true, theme)
            .on_hover_text("Switch between Dark Mode (Matte Obsidian) and Light Mode")
            .clicked()
        {
            config.window.theme_mode = match config.window.theme_mode {
                ThemeMode::Dark => ThemeMode::Light,
                ThemeMode::Light => ThemeMode::Dark,
            };
            changed = true;
        }

        let any_labels = config.buttons.iter().any(|b| b.show_label);
        let label_btn_text = if any_labels { "Labels: On" } else { "Labels: Off" };
        if pill_button_with_icon(
            ui,
            Some(ToolbarIcon::TextLabel),
            label_btn_text,
            any_labels,
            false,
            false,
            true,
            theme,
        )
        .on_hover_text("Toggle text titles on all buttons (Icon + Title vs Icon-Only)")
        .clicked()
        {
            let target = !any_labels;
            for b in &mut config.buttons {
                b.show_label = target;
            }
            changed = true;
        }

        // Mute All Outputs (Speaker icon differentiates output mute from mic mute)
        let out_muted = audio.are_outputs_muted();
        let out_mute_label = if out_muted { "Out Muted" } else { "Mute Out" };
        if pill_button_with_icon(
            ui,
            Some(ToolbarIcon::Speaker { muted: out_muted }),
            out_mute_label,
            out_muted,
            false,
            out_muted,
            true,
            theme,
        )
        .on_hover_text("Mute or unmute all active audio output devices (Speakers / Headphones)")
        .clicked()
        {
            let _ = audio.toggle_mute_outputs();
        }

        // Mute All Microphones (Studio Mic icon differentiates mic mute from speaker mute)
        let in_muted = audio.are_inputs_muted();
        let in_mute_label = if in_muted { "Mic Muted" } else { "Mute Mic" };
        if pill_button_with_icon(
            ui,
            Some(ToolbarIcon::Microphone { muted: in_muted }),
            in_mute_label,
            in_muted,
            false,
            in_muted,
            true,
            theme,
        )
        .on_hover_text("Mute or unmute all active microphone input devices")
        .clicked()
        {
            let _ = audio.toggle_mute_inputs();
        }

        ui.add_space(3.0);
        ui.separator();
        ui.add_space(3.0);

        // ── 2. MIDDLE GROUP: Columns Stepper (Grid Icon + Columns − N +) ────────
        let (grid_icon_rect, _) = ui.allocate_exact_size(Vec2::new(16.0, 26.0), Sense::hover());
        draw_toolbar_icon(
            ui.painter(),
            grid_icon_rect.center(),
            ToolbarIcon::ColumnsGrid,
            muted_text,
        );
        ui.label(RichText::new("Columns").size(12.0).color(muted_text));
        if pill_button(ui, "−", false, false, theme)
            .on_hover_text("Fewer buttons per row")
            .clicked()
            && config.window.columns > 1
        {
            config.window.columns -= 1;
            changed = true;
        }
        ui.add_space(1.0);
        ui.label(
            RichText::new(format!("{}", config.window.columns))
                .size(13.0)
                .strong(),
        );
        ui.add_space(1.0);
        if pill_button(ui, "+", false, false, theme)
            .on_hover_text("More buttons per row")
            .clicked()
            && config.window.columns < 12
        {
            config.window.columns += 1;
            changed = true;
        }

        ui.add_space(3.0);
        ui.separator();
        ui.add_space(3.0);

        // ── 3. RIGHT GROUP: Add Button & Minimalist Mode ─────────────────────────
        if pill_button_with_icon(
            ui,
            Some(ToolbarIcon::Plus),
            "Add Button",
            false,
            true,
            false,
            true,
            theme,
        )
        .on_hover_text("Bind a new audio input or output button (opens in an external dialog window)")
        .clicked()
        {
            let new_btn = create_new_button_from_audio(audio, config.buttons.len());
            config.buttons.push(new_btn);
            editor.selected_button_idx = Some(config.buttons.len() - 1);
            changed = true;
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if pill_button_with_icon(
                ui,
                Some(ToolbarIcon::Minimalist),
                "Minimalist",
                false,
                true,
                false,
                true,
                theme,
            )
            .on_hover_text(
                "Strip all window bars and show strictly the button grid.\n• Right-click anywhere for quick context window (Add Button, Mute All, etc.)\n• Drag buttons to reorder\n• Alt+Drag or Right-Drag to move window\n• Click ⋮ on any button to customize",
            )
            .clicked()
            {
                *enter_minimalist_requested = true;
                changed = true;
            }
        });
    });

    changed
}

/// Fixed dimensions for the Customize Button dialog (non-resizable).
const CUSTOMIZE_DIALOG_WIDTH: f32 = 488.0;
const CUSTOMIZE_DIALOG_HEIGHT: f32 = 656.0;

/// Renders the per-button customization dialog inside an **independent native OS viewport**
/// (`ctx.show_viewport_immediate`) positioned on the **same monitor** as the main app window.
pub fn render_button_editor_modal(
    ctx: &Context,
    main_hwnd: Option<isize>,
    config: &mut AppConfig,
    audio: &mut AudioEngine,
    editor: &mut EditorState,
    icon_cache: &mut IconCache,
) -> bool {
    let Some(idx) = editor.selected_button_idx else {
        editor.was_open = false;
        editor.dialog_spawn_pos = None;
        return false;
    };
    if idx >= config.buttons.len() {
        editor.selected_button_idx = None;
        editor.was_open = false;
        editor.dialog_spawn_pos = None;
        return false;
    }

    let dialog_w = CUSTOMIZE_DIALOG_WIDTH;
    let dialog_h = CUSTOMIZE_DIALOG_HEIGHT;
    let just_opened = !editor.was_open;

    if just_opened || editor.dialog_spawn_pos.is_none() {
        let pos = main_hwnd
            .and_then(|hwnd| {
                compute_same_monitor_dialog_pos(hwnd, dialog_w, dialog_h, ctx.pixels_per_point())
            })
            .or_else(|| {
                let x = config.window.x?;
                let y = config.window.y?;
                Some([
                    x + (config.window.width - dialog_w) * 0.5,
                    (y - 60.0).max(-4000.0),
                ])
            });
        editor.dialog_spawn_pos = pos;
        editor.was_open = true;
    }

    let mut changed = false;
    let mut close_dialog = false;
    let mut delete_requested = false;
    let mut move_delta: isize = 0;

    let total_buttons = config.buttons.len();
    let theme = config.window.theme_mode;

    let viewport_id = ViewportId::from_hash_of("lyre_external_button_editor_viewport");
    let mut viewport_builder = ViewportBuilder::default()
        .with_title(format!("Lyre — Customize Button #{}", idx + 1))
        .with_inner_size([dialog_w, dialog_h])
        .with_min_inner_size([dialog_w, dialog_h])
        .with_max_inner_size([dialog_w, dialog_h])
        .with_always_on_top()
        .with_taskbar(false)
        .with_maximize_button(false)
        .with_resizable(false);

    if let Some(icon) = crate::tray::load_embedded_egui_icon() {
        viewport_builder = viewport_builder.with_icon(std::sync::Arc::new(icon));
    }

    if let Some(pos) = editor.dialog_spawn_pos {
        viewport_builder = viewport_builder.with_position(Pos2::new(pos[0], pos[1]));
    }

    let spawn_pos_for_first_frame = if just_opened {
        editor.dialog_spawn_pos
    } else {
        None
    };

    ctx.show_viewport_immediate(viewport_id, viewport_builder, |vp_ctx, class| {
        apply_theme(vp_ctx, theme);

        // Lock window to fixed non-resizable dimensions
        if just_opened {
            vp_ctx.send_viewport_cmd(ViewportCommand::Resizable(false));
            vp_ctx.send_viewport_cmd(ViewportCommand::InnerSize(Vec2::new(dialog_w, dialog_h)));
        }

        if let Some(pos) = spawn_pos_for_first_frame {
            vp_ctx.send_viewport_cmd(ViewportCommand::OuterPosition(Pos2::new(pos[0], pos[1])));
        }

        if vp_ctx.input(|i| i.viewport().close_requested()) {
            close_dialog = true;
        }

        let (panel_bg, card_bg, card_border, label_muted) = match theme {
            ThemeMode::Dark => (
                Color32::from_rgb(13, 14, 17),   // #0D0E11 carbon canvas
                Color32::from_rgb(20, 22, 27),   // #14161B matte card
                Color32::from_rgb(36, 39, 48),   // #242730 hairline border
                Color32::from_rgb(142, 148, 164),
            ),
            ThemeMode::Light => (
                Color32::from_rgb(244, 245, 248),
                Color32::from_rgb(255, 255, 255),
                Color32::from_rgb(218, 222, 232),
                Color32::from_rgb(100, 105, 118),
            ),
        };

        let section_card_frame = EguiFrame::none()
            .fill(card_bg)
            .stroke(Stroke::new(1.0, card_border))
            .rounding(Rounding::same(10.0))
            .inner_margin(Margin::symmetric(12.0, 10.0));

        let render_contents = |ui: &mut Ui| {
            ScrollArea::vertical().show(ui, |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(8.0, 9.0);
                let btn = &mut config.buttons[idx];
                let [ar, ag, ab] = parse_hex_color(&btn.color_hex).unwrap_or([0x06, 0xB6, 0xD4]);
                let accent_color = Color32::from_rgb(ar, ag, ab);

                // =========================================================================
                // SECTION 1: AUDIO ROUTING (Device Type + Bound Endpoint)
                // =========================================================================
                section_card_frame.show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(
                        RichText::new("AUDIO ROUTING")
                            .size(10.5)
                            .strong()
                            .color(label_muted),
                    );
                    ui.add_space(2.0);

                    // Segmented Output vs Input cards with icons
                    ui.columns(2, |cols| {
                        if draw_direction_segment_tile(
                            &mut cols[0],
                            "Output Device",
                            "Speakers, Headphones, DAC",
                            PresetIcon::Headphones,
                            btn.direction == EndpointDirection::Output,
                            accent_color,
                            theme,
                            icon_cache,
                        )
                        .clicked()
                            && btn.direction != EndpointDirection::Output
                        {
                            btn.direction = EndpointDirection::Output;
                            if let Some(first) = audio
                                .snapshot()
                                .outputs
                                .iter()
                                .find(|ep| !crate::audio::is_sonar_virtual_name(&ep.name))
                                .or_else(|| audio.snapshot().outputs.first())
                            {
                                btn.device_id = first.id.clone();
                                btn.title = shorten_device_title(&first.name);
                                btn.icon = IconSource::Preset(PresetIcon::infer_from_name(
                                    &first.name,
                                    EndpointDirection::Output,
                                ));
                            }
                            changed = true;
                        }

                        if draw_direction_segment_tile(
                            &mut cols[1],
                            "Input Device",
                            "Studio Mic, Headset Mic",
                            PresetIcon::Microphone,
                            btn.direction == EndpointDirection::Input,
                            accent_color,
                            theme,
                            icon_cache,
                        )
                        .clicked()
                            && btn.direction != EndpointDirection::Input
                        {
                            btn.direction = EndpointDirection::Input;
                            if let Some(first) = audio
                                .snapshot()
                                .inputs
                                .iter()
                                .find(|ep| !crate::audio::is_sonar_virtual_name(&ep.name))
                                .or_else(|| audio.snapshot().inputs.first())
                            {
                                btn.device_id = first.id.clone();
                                btn.title = shorten_device_title(&first.name);
                                btn.icon = IconSource::Preset(PresetIcon::infer_from_name(
                                    &first.name,
                                    EndpointDirection::Input,
                                ));
                            }
                            changed = true;
                        }
                    });

                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("Bound Windows Audio Endpoint")
                            .size(12.0)
                            .color(label_muted),
                    );

                    let endpoints = match btn.direction {
                        EndpointDirection::Output => &audio.snapshot().outputs,
                        EndpointDirection::Input => &audio.snapshot().inputs,
                    };
                    let current_ep_name = endpoints
                        .iter()
                        .find(|ep| ep.id == btn.device_id)
                        .map(|ep| ep.name.clone())
                        .unwrap_or_else(|| "Unbound / Disconnected Device".to_string());

                    egui::ComboBox::from_id_salt("endpoint_selector_combo")
                        .width(ui.available_width())
                        .selected_text(RichText::new(current_ep_name).size(13.0))
                        .show_ui(ui, |ui| {
                            for ep in endpoints {
                                let is_selected = ep.id == btn.device_id;
                                let is_active_now = audio.is_device_active(&ep.id, btn.direction);
                                let label = if is_active_now {
                                    format!("● {} (Active)", ep.name)
                                } else {
                                    ep.name.clone()
                                };
                                if ui.selectable_label(is_selected, label).clicked() {
                                    btn.device_id = ep.id.clone();
                                    btn.title = shorten_device_title(&ep.name);
                                    btn.icon = IconSource::Preset(PresetIcon::infer_from_name(
                                        &ep.name,
                                        btn.direction,
                                    ));
                                    changed = true;
                                }
                            }
                        });
                });

                // =========================================================================
                // SECTION 2: LABEL, BADGE & ACCENT COLOR
                // =========================================================================
                section_card_frame.show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(
                        RichText::new("BUTTON APPEARANCE & COLOR")
                            .size(10.5)
                            .strong()
                            .color(label_muted),
                    );
                    ui.add_space(2.0);

                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Title:").size(12.5));
                        let resp = ui.add_sized(
                            [ui.available_width(), 24.0],
                            egui::TextEdit::singleline(&mut btn.title),
                        );
                        if resp.changed() {
                            changed = true;
                        }
                    });

                    ui.add_space(2.0);
                    ui.horizontal(|ui| {
                        if draw_toggle_switch(
                            ui,
                            &mut btn.show_label,
                            "Show Text Title",
                            accent_color,
                            theme,
                        ) {
                            changed = true;
                        }
                        ui.add_space(14.0);
                        if draw_toggle_switch(
                            ui,
                            &mut btn.badge_enabled,
                            "Corner Number Badge",
                            accent_color,
                            theme,
                        ) {
                            changed = true;
                        }
                        if btn.badge_enabled {
                            if ui
                                .add(
                                    egui::DragValue::new(&mut btn.badge_number)
                                        .range(1..=99)
                                        .speed(0.2),
                                )
                                .changed()
                            {
                                changed = true;
                            }
                        }
                    });

                    ui.add_space(2.0);
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new("Accent Color:").size(12.5));
                        let mut rgb = parse_hex_color(&btn.color_hex).unwrap_or([0x3B, 0x82, 0xF6]);
                        if ui.color_edit_button_srgb(&mut rgb).changed() {
                            btn.color_hex = format_hex_color(rgb);
                            changed = true;
                        }
                        ui.monospace(RichText::new(&btn.color_hex).size(12.0));

                        ui.add_space(4.0);
                        for &swatch_hex in &DEFAULT_COLOR_PALETTE {
                            if let Some([r, g, b]) = parse_hex_color(swatch_hex) {
                                let c = Color32::from_rgb(r, g, b);
                                let is_sel = btn.color_hex.eq_ignore_ascii_case(swatch_hex);
                                let (rect, resp) =
                                    ui.allocate_exact_size(Vec2::new(20.0, 20.0), Sense::click());
                                if resp.hovered() {
                                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                                }
                                // Uniform matte swatch (zero two-tone band)
                                ui.painter().rect_filled(rect, 6.0, c);
                                if is_sel {
                                    ui.painter().rect_stroke(
                                        rect.expand(1.5),
                                        7.0,
                                        Stroke::new(1.8, Color32::WHITE),
                                    );
                                }
                                if resp.clicked() {
                                    btn.color_hex = swatch_hex.to_string();
                                    changed = true;
                                }
                            }
                        }
                    });
                });

                // =========================================================================
                // SECTION 3: ICON PRESETS + PROMINENT CUSTOM IMAGE UPLOADER
                // =========================================================================
                section_card_frame.show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(
                        RichText::new("BUILT-IN STUDIO ICON PRESETS")
                            .size(10.5)
                            .strong()
                            .color(label_muted),
                    );
                    ui.add_space(2.0);

                    // Visual Grid of the 7 Built-In Vector Presets (with actual SVG icons!)
                    let avail_w = ui.available_width();
                    let cols_count = 4usize;
                    let spacing = 6.0;
                    let tile_w = ((avail_w - spacing * (cols_count as f32 - 1.0))
                        / cols_count as f32)
                        .max(72.0);
                    let tile_h = 54.0;

                    for row in PresetIcon::ALL.chunks(cols_count) {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = spacing;
                            for &preset in row {
                                let is_sel = btn.icon == IconSource::Preset(preset);
                                if draw_preset_icon_tile(
                                    ui,
                                    Vec2::new(tile_w, tile_h),
                                    preset,
                                    is_sel,
                                    accent_color,
                                    theme,
                                    icon_cache,
                                )
                                .clicked()
                                {
                                    btn.icon = IconSource::Preset(preset);
                                    changed = true;
                                }
                            }
                        });
                    }

                    ui.add_space(6.0);
                    ui.label(
                        RichText::new("CUSTOM IMAGE")
                            .size(10.5)
                            .strong()
                            .color(label_muted),
                    );
                    ui.add_space(2.0);

                    // Prominent, Visually Distinct Custom Image Upload Button Card
                    if draw_prominent_upload_button(
                        ui,
                        &btn.icon,
                        accent_color,
                        theme,
                        icon_cache,
                    )
                    .clicked()
                    {
                        if let Some(path) = rfd::FileDialog::new()
                            .set_title("Select Custom Button Image or Icon")
                            .add_filter(
                                "Images & Icons (.png, .webp, .svg, .ico, .jpg)",
                                &["png", "webp", "svg", "ico", "jpg", "jpeg", "bmp"],
                            )
                            .pick_file()
                        {
                            btn.icon = IconSource::CustomPath(path);
                            icon_cache.clear();
                            changed = true;
                        }
                    }
                });

                // =========================================================================
                // SECTION 4: BOTTOM ACTION BAR
                // =========================================================================
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;

                    if pill_button_with_icon(
                        ui,
                        Some(ToolbarIcon::ArrowLeft),
                        "Move Left",
                        false,
                        false,
                        false,
                        idx > 0,
                        theme,
                    )
                    .clicked()
                        && idx > 0
                    {
                        move_delta = -1;
                    }

                    if pill_button_with_icon(
                        ui,
                        Some(ToolbarIcon::ArrowRight),
                        "Move Right",
                        false,
                        false,
                        false,
                        idx + 1 < total_buttons,
                        theme,
                    )
                    .clicked()
                        && idx + 1 < total_buttons
                    {
                        move_delta = 1;
                    }

                    if pill_button_with_icon(
                        ui,
                        Some(ToolbarIcon::Trash),
                        "Delete Button",
                        false,
                        false,
                        true,
                        true,
                        theme,
                    )
                    .clicked()
                    {
                        delete_requested = true;
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if pill_button_with_icon(
                            ui,
                            Some(ToolbarIcon::Check),
                            "Done",
                            true,
                            true,
                            false,
                            true,
                            theme,
                        )
                        .clicked()
                        {
                            close_dialog = true;
                        }
                    });
                });
            });
        };

        if class == ViewportClass::Embedded {
            let mut open = true;
            egui::Window::new(format!("Customize Button #{}", idx + 1))
                .open(&mut open)
                .show(vp_ctx, render_contents);
            if !open {
                close_dialog = true;
            }
        } else {
            CentralPanel::default()
                .frame(
                    EguiFrame::none()
                        .fill(panel_bg)
                        .inner_margin(Margin::symmetric(14.0, 12.0)),
                )
                .show(vp_ctx, render_contents);
        }
    });

    if close_dialog {
        editor.selected_button_idx = None;
        editor.was_open = false;
        editor.dialog_spawn_pos = None;
    }

    if delete_requested {
        config.buttons.remove(idx);
        editor.selected_button_idx = None;
        editor.was_open = false;
        editor.dialog_spawn_pos = None;
        changed = true;
    } else if move_delta != 0 {
        let new_idx =
            ((idx as isize) + move_delta).clamp(0, (total_buttons as isize) - 1) as usize;
        if new_idx != idx {
            config.buttons.swap(idx, new_idx);
            editor.selected_button_idx = Some(new_idx);
            changed = true;
        }
    }

    changed
}

/// Draws a uniform matte segmented card for selecting `Output Device` vs `Input Device`.
#[allow(clippy::too_many_arguments)]
fn draw_direction_segment_tile(
    ui: &mut Ui,
    title: &str,
    subtitle: &str,
    icon_preset: PresetIcon,
    selected: bool,
    accent: Color32,
    theme: ThemeMode,
    icon_cache: &mut IconCache,
) -> egui::Response {
    let desired = Vec2::new(ui.available_width(), 44.0);
    let (rect, resp) = ui.allocate_exact_size(desired, Sense::click());
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }

    let [ar, ag, ab, _] = accent.to_array();
    let (bg, border, title_col, sub_col) = match theme {
        ThemeMode::Dark => {
            if selected {
                (
                    Color32::from_rgba_unmultiplied(ar, ag, ab, 28),
                    accent,
                    Color32::WHITE,
                    Color32::from_rgb(188, 194, 210),
                )
            } else if resp.hovered() {
                (
                    Color32::from_rgb(28, 31, 38),
                    Color32::from_rgb(64, 69, 82),
                    Color32::from_rgb(230, 232, 240),
                    Color32::from_rgb(144, 150, 166),
                )
            } else {
                (
                    Color32::from_rgb(16, 18, 22),
                    Color32::from_rgb(38, 41, 50),
                    Color32::from_rgb(202, 206, 218),
                    Color32::from_rgb(126, 132, 146),
                )
            }
        }
        ThemeMode::Light => {
            if selected {
                (
                    Color32::from_rgba_unmultiplied(ar, ag, ab, 26),
                    accent,
                    Color32::from_rgb(18, 20, 26),
                    Color32::from_rgb(70, 74, 86),
                )
            } else {
                (
                    Color32::from_rgb(245, 246, 250),
                    Color32::from_rgb(210, 214, 224),
                    Color32::from_rgb(40, 42, 50),
                    Color32::from_rgb(110, 114, 126),
                )
            }
        }
    };

    let rounding = Rounding::same(8.0);
    ui.painter().rect_filled(rect, rounding, bg);
    ui.painter().rect_stroke(
        rect,
        rounding,
        Stroke::new(if selected { 1.5 } else { 1.0 }, border),
    );

    let icon_rect = Rect::from_center_size(
        Pos2::new(rect.left() + 21.0, rect.center().y),
        Vec2::splat(20.0),
    );
    if let Some(tex) = icon_cache.get_texture(ui.ctx(), &IconSource::Preset(icon_preset), 32.0) {
        ui.painter().image(
            tex.id(),
            icon_rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            if selected { accent } else { title_col },
        );
    }

    ui.painter().text(
        Pos2::new(rect.left() + 38.0, rect.center().y - 7.0),
        Align2::LEFT_CENTER,
        title,
        FontId::new(12.5, FontFamily::Proportional),
        title_col,
    );
    ui.painter().text(
        Pos2::new(rect.left() + 38.0, rect.center().y + 8.0),
        Align2::LEFT_CENTER,
        subtitle,
        FontId::new(10.0, FontFamily::Proportional),
        sub_col,
    );

    resp
}

/// Draws a uniform matte visual icon tile for one of the 7 built-in studio SVG presets.
fn draw_preset_icon_tile(
    ui: &mut Ui,
    size: Vec2,
    preset: PresetIcon,
    selected: bool,
    accent: Color32,
    theme: ThemeMode,
    icon_cache: &mut IconCache,
) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }

    let [ar, ag, ab, _] = accent.to_array();
    let (bg, border, icon_tint, text_col) = match theme {
        ThemeMode::Dark => {
            if selected {
                (
                    Color32::from_rgba_unmultiplied(ar, ag, ab, 32),
                    accent,
                    Color32::WHITE,
                    Color32::WHITE,
                )
            } else if resp.hovered() {
                (
                    Color32::from_rgb(28, 31, 38),
                    Color32::from_rgb(68, 73, 86),
                    Color32::from_rgb(240, 242, 248),
                    Color32::from_rgb(230, 232, 240),
                )
            } else {
                (
                    Color32::from_rgb(16, 18, 22),
                    Color32::from_rgb(38, 41, 50),
                    Color32::from_rgb(186, 192, 206),
                    Color32::from_rgb(172, 178, 192),
                )
            }
        }
        ThemeMode::Light => {
            if selected {
                (
                    Color32::from_rgba_unmultiplied(ar, ag, ab, 28),
                    accent,
                    Color32::from_rgb(18, 20, 26),
                    Color32::from_rgb(18, 20, 26),
                )
            } else {
                (
                    Color32::from_rgb(245, 246, 250),
                    Color32::from_rgb(210, 214, 224),
                    Color32::from_rgb(55, 58, 68),
                    Color32::from_rgb(65, 68, 78),
                )
            }
        }
    };

    let rounding = Rounding::same(8.0);
    // Uniform matte tile fill (zero two-tone band)
    ui.painter().rect_filled(rect, rounding, bg);
    ui.painter().rect_stroke(
        rect,
        rounding,
        Stroke::new(if selected { 1.5 } else { 1.0 }, border),
    );

    let icon_rect = Rect::from_center_size(
        Pos2::new(rect.center().x, rect.top() + 19.0),
        Vec2::splat(22.0),
    );
    if let Some(tex) = icon_cache.get_texture(ui.ctx(), &IconSource::Preset(preset), 48.0) {
        ui.painter().image(
            tex.id(),
            icon_rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            icon_tint,
        );
    }

    ui.painter().text(
        Pos2::new(rect.center().x, rect.bottom() - 10.0),
        Align2::CENTER_CENTER,
        preset.display_name(),
        FontId::new(10.5, FontFamily::Proportional),
        text_col,
    );

    resp
}

/// Draws a prominent, visually distinct full-width matte card for uploading a custom image
/// (`.png`, `.webp`, `.svg`, `.ico`, `.jpg`), including a live thumbnail preview and
/// full multi-line text wrapping (`break_anywhere = true`) for long filenames and file paths.
fn draw_prominent_upload_button(
    ui: &mut Ui,
    current_icon: &IconSource,
    accent: Color32,
    theme: ThemeMode,
    icon_cache: &mut IconCache,
) -> egui::Response {
    let avail_w = ui.available_width();
    let is_custom = matches!(current_icon, IconSource::CustomPath(_));
    let [ar, ag, ab, _] = accent.to_array();

    let (title_col, sub_col) = match theme {
        ThemeMode::Dark => (Color32::WHITE, Color32::from_rgb(168, 175, 192)),
        ThemeMode::Light => (
            Color32::from_rgb(18, 20, 26),
            Color32::from_rgb(75, 80, 94),
        ),
    };

    let (primary_text, secondary_text) = if let IconSource::CustomPath(ref path) = current_icon {
        let fname = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("custom_image");
        (
            format!("Custom Image: {}", fname),
            path.display().to_string(),
        )
    } else {
        (
            "Upload Custom Image…".to_string(),
            "Supports transparent .PNG, .WEBP, .SVG, .ICO, .JPG".to_string(),
        )
    };

    // Wrap both filename and file path cleanly between the left thumbnail (56px) and right CTA button (88px)
    let text_max_width = (avail_w - 56.0 - 88.0).max(110.0);

    let mut primary_job = egui::text::LayoutJob::simple(
        primary_text,
        FontId::new(12.2, FontFamily::Proportional),
        title_col,
        text_max_width,
    );
    primary_job.wrap.break_anywhere = true;
    let primary_galley = ui.fonts(|f| f.layout_job(primary_job));

    let mut secondary_job = egui::text::LayoutJob::simple(
        secondary_text,
        FontId::new(10.4, FontFamily::Proportional),
        sub_col,
        text_max_width,
    );
    secondary_job.wrap.break_anywhere = true;
    let secondary_galley = ui.fonts(|f| f.layout_job(secondary_job));

    let text_block_h = primary_galley.size().y + 3.0 + secondary_galley.size().y;
    let card_h = (text_block_h + 16.0).max(56.0);

    let desired = Vec2::new(avail_w, card_h);
    let (rect, resp) = ui.allocate_exact_size(desired, Sense::click());
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }

    let (bg_fill, border_col, badge_bg) = match theme {
        ThemeMode::Dark => {
            let bg = if resp.hovered() {
                Color32::from_rgba_unmultiplied(ar, ag, ab, 32)
            } else if is_custom {
                Color32::from_rgba_unmultiplied(ar, ag, ab, 24)
            } else {
                Color32::from_rgba_unmultiplied(ar, ag, ab, 15)
            };
            let border = if resp.hovered() || is_custom {
                accent
            } else {
                Color32::from_rgba_unmultiplied(ar, ag, ab, 145)
            };
            (
                bg,
                border,
                Color32::from_rgba_unmultiplied(ar, ag, ab, 48),
            )
        }
        ThemeMode::Light => {
            let bg = if resp.hovered() {
                Color32::from_rgba_unmultiplied(ar, ag, ab, 32)
            } else {
                Color32::from_rgba_unmultiplied(ar, ag, ab, 16)
            };
            (
                bg,
                accent,
                Color32::from_rgba_unmultiplied(ar, ag, ab, 36),
            )
        }
    };

    let rounding = Rounding::same(9.0);
    // Uniform matte background (zero two-tone band)
    ui.painter().rect_filled(rect, rounding, bg_fill);
    ui.painter().rect_stroke(
        rect,
        rounding,
        Stroke::new(if is_custom || resp.hovered() { 1.6 } else { 1.1 }, border_col),
    );

    // Left icon container (shows either live custom image thumbnail or an Upload vector icon)
    let thumb_box = Rect::from_center_size(
        Pos2::new(rect.left() + 29.0, rect.center().y),
        Vec2::splat(36.0),
    );
    ui.painter()
        .rect_filled(thumb_box, Rounding::same(7.0), badge_bg);

    if is_custom {
        if let Some(tex) = icon_cache.get_texture(ui.ctx(), current_icon, 64.0) {
            ui.painter().image(
                tex.id(),
                thumb_box.shrink(3.0),
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        }
    } else {
        // Draw crisp vector Upload Arrow + Tray icon
        let p = ui.painter();
        let c = thumb_box.center();
        let stroke = Stroke::new(1.9, Color32::WHITE);
        p.line_segment([Pos2::new(c.x, c.y + 5.0), Pos2::new(c.x, c.y - 7.0)], stroke);
        p.line_segment(
            [Pos2::new(c.x - 4.5, c.y - 2.5), Pos2::new(c.x, c.y - 7.0)],
            stroke,
        );
        p.line_segment(
            [Pos2::new(c.x + 4.5, c.y - 2.5), Pos2::new(c.x, c.y - 7.0)],
            stroke,
        );
        p.line_segment(
            [Pos2::new(c.x - 7.0, c.y + 4.0), Pos2::new(c.x - 7.0, c.y + 8.0)],
            stroke,
        );
        p.line_segment(
            [Pos2::new(c.x - 7.0, c.y + 8.0), Pos2::new(c.x + 7.0, c.y + 8.0)],
            stroke,
        );
        p.line_segment(
            [Pos2::new(c.x + 7.0, c.y + 8.0), Pos2::new(c.x + 7.0, c.y + 4.0)],
            stroke,
        );
    }

    let text_top_y = rect.center().y - text_block_h * 0.5;
    let text_left_x = rect.left() + 56.0;
    ui.painter().galley(
        Pos2::new(text_left_x, text_top_y),
        primary_galley.clone(),
        title_col,
    );
    ui.painter().galley(
        Pos2::new(text_left_x, text_top_y + primary_galley.size().y + 3.0),
        secondary_galley,
        sub_col,
    );

    // Right-side "Browse…" / "Change" pill badge inside the upload card
    let cta_rect = Rect::from_center_size(
        Pos2::new(rect.right() - 44.0, rect.center().y),
        Vec2::new(68.0, 24.0),
    );
    ui.painter()
        .rect_filled(cta_rect, Rounding::same(6.0), accent);
    ui.painter().text(
        cta_rect.center(),
        Align2::CENTER_CENTER,
        if is_custom { "Change" } else { "Browse…" },
        FontId::new(11.5, FontFamily::Proportional),
        Color32::WHITE,
    );

    resp
}

/// Renders the custom **Linear / Raycast Matte Obsidian** right-click context window
/// inside an independent frameless native OS viewport (`ctx.show_viewport_immediate`).
/// Because it is a standalone OS popup window, it extends freely outside even a tiny main window.
pub fn render_context_menu_viewport(
    ctx: &Context,
    config: &AppConfig,
    audio: &AudioEngine,
    editor: &mut EditorState,
) -> ContextMenuAction {
    let Some(mut menu_state) = editor.context_menu.clone() else {
        return ContextMenuAction::None;
    };

    // Validate button index if one was targeted
    if let Some(idx) = menu_state.target_button_idx {
        if idx >= config.buttons.len() {
            menu_state.target_button_idx = None;
        }
    }

    let has_target_btn = menu_state.target_button_idx.is_some();
    let menu_w = 292.0;
    let menu_h = if has_target_btn { 504.0 } else { 356.0 };
    let theme = config.window.theme_mode;

    let viewport_id = ViewportId::from_hash_of("lyre_context_menu_viewport");
    let viewport_builder = ViewportBuilder::default()
        .with_title("Lyre — Quick Context Menu")
        .with_decorations(false)
        .with_taskbar(false)
        .with_always_on_top()
        .with_resizable(false)
        .with_inner_size([menu_w, menu_h])
        .with_position(Pos2::new(menu_state.spawn_pos[0], menu_state.spawn_pos[1]));

    let mut chosen_action = ContextMenuAction::None;
    let mut should_close = false;
    let just_opened = menu_state.just_opened;
    let spawn_pos = menu_state.spawn_pos;

    let outputs_muted = audio.are_outputs_muted();
    let inputs_muted = audio.are_inputs_muted();
    let minimalist_mode = config.window.minimalist_mode;
    let dark_mode = theme == ThemeMode::Dark;
    let labels_shown = config.buttons.iter().any(|b| b.show_label);
    let always_on_top = config.window.always_on_top;

    let target_btn_info = menu_state.target_button_idx.and_then(|idx| {
        config.buttons.get(idx).map(|b| {
            let [r, g, b_ch] = parse_hex_color(&b.color_hex).unwrap_or([59, 130, 246]);
            let accent = Color32::from_rgb(r, g, b_ch);
            let is_active = audio.is_device_active(&b.device_id, b.direction);
            (idx, b.title.clone(), b.direction, accent, is_active)
        })
    });

    ctx.show_viewport_immediate(viewport_id, viewport_builder, |vp_ctx, _class| {
        apply_theme(vp_ctx, theme);

        if just_opened {
            vp_ctx.send_viewport_cmd(ViewportCommand::OuterPosition(Pos2::new(
                spawn_pos[0],
                spawn_pos[1],
            )));
            vp_ctx.send_viewport_cmd(ViewportCommand::InnerSize(Vec2::new(menu_w, menu_h)));
            vp_ctx.send_viewport_cmd(ViewportCommand::Focus);
        }

        let focused = vp_ctx.input(|i| i.viewport().focused);
        if focused == Some(true) {
            menu_state.had_focus = true;
        }
        if vp_ctx.input(|i| i.viewport().close_requested() || i.key_pressed(egui::Key::Escape)) {
            should_close = true;
        } else if (menu_state.had_focus
            || menu_state.opened_at.elapsed() > Duration::from_millis(320))
            && focused == Some(false)
        {
            should_close = true;
        }

        let (canvas_bg, outer_border, card_bg, card_border, section_label_col, divider_col) =
            match theme {
                ThemeMode::Dark => (
                    Color32::from_rgb(13, 14, 18),   // #0D0E12 deep obsidian canvas
                    Color32::from_rgb(44, 49, 62),   // #2C313E crisp outer 1px border
                    Color32::from_rgb(19, 21, 27),   // #13151B uniform matte card
                    Color32::from_rgb(34, 38, 48),   // #222630 card hairline border
                    Color32::from_rgb(122, 128, 144),
                    Color32::from_rgb(32, 36, 45),
                ),
                ThemeMode::Light => (
                    Color32::from_rgb(245, 246, 250),
                    Color32::from_rgb(198, 204, 218),
                    Color32::from_rgb(255, 255, 255),
                    Color32::from_rgb(220, 224, 234),
                    Color32::from_rgb(104, 110, 124),
                    Color32::from_rgb(222, 226, 236),
                ),
            };

        let default_accent = target_btn_info
            .as_ref()
            .map(|(_, _, _, acc, _)| *acc)
            .unwrap_or(Color32::from_rgb(59, 130, 246));

        CentralPanel::default()
            .frame(
                EguiFrame::none()
                    .fill(canvas_bg)
                    .stroke(Stroke::new(1.0, outer_border))
                    .inner_margin(Margin::symmetric(10.0, 9.0)),
            )
            .show(vp_ctx, |ui| {
                // Subtle 1px top-edge specular rim line on the context window
                let full_rect = ui.max_rect();
                if theme == ThemeMode::Dark {
                    ui.painter().line_segment(
                        [
                            Pos2::new(full_rect.left() + 8.0, full_rect.top() + 0.5),
                            Pos2::new(full_rect.right() - 8.0, full_rect.top() + 0.5),
                        ],
                        Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 18)),
                    );
                }

                // ── Top Header Bar (Draggable + Close '×' Pill) ────────────────────
                ui.horizontal(|ui| {
                    if let Some((_, ref title, dir, accent, is_active)) = target_btn_info {
                        let dot_col = if is_active {
                            Color32::from_rgb(34, 197, 94)
                        } else {
                            accent
                        };
                        let (dot_rect, _) =
                            ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
                        ui.painter()
                            .circle_filled(dot_rect.center(), 4.2, dot_col);
                        ui.label(
                            RichText::new(shorten_device_title(title))
                                .size(12.5)
                                .strong(),
                        );
                        let dir_tag = match dir {
                            EndpointDirection::Output => "OUT",
                            EndpointDirection::Input => "MIC",
                        };
                        ui.label(
                            RichText::new(dir_tag)
                                .size(9.5)
                                .strong()
                                .color(section_label_col),
                        );
                    } else {
                        let (dot_rect, _) =
                            ui.allocate_exact_size(Vec2::splat(9.0), Sense::hover());
                        ui.painter().circle_filled(
                            dot_rect.center(),
                            3.6,
                            Color32::from_rgb(59, 130, 246),
                        );
                        ui.label(
                            RichText::new("LYRE QUICK ACTIONS")
                                .size(10.5)
                                .strong()
                                .color(section_label_col),
                        );
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let (close_rect, close_resp) =
                            ui.allocate_exact_size(Vec2::new(22.0, 20.0), Sense::click());
                        if close_resp.hovered() {
                            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                            ui.painter().rect_filled(
                                close_rect,
                                Rounding::same(5.0),
                                if theme == ThemeMode::Dark {
                                    Color32::from_rgb(34, 38, 48)
                                } else {
                                    Color32::from_rgb(226, 230, 240)
                                },
                            );
                        }
                        ui.painter().text(
                            close_rect.center(),
                            Align2::CENTER_CENTER,
                            "×",
                            FontId::new(14.0, FontFamily::Proportional),
                            section_label_col,
                        );
                        if close_resp.clicked() {
                            should_close = true;
                        }
                    });
                });

                ui.add_space(6.0);

                let group_frame = EguiFrame::none()
                    .fill(card_bg)
                    .stroke(Stroke::new(1.0, card_border))
                    .rounding(Rounding::same(8.0))
                    .inner_margin(Margin::symmetric(6.0, 5.0));

                // ── Section 1: Target Button Actions (when right-clicking a button) ──
                if let Some((idx, _, _, accent, is_active)) = target_btn_info {
                    group_frame.show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;

                        let switch_badge = if is_active { "ACTIVE" } else { "Switch" };
                        if draw_context_action_row(
                            ui,
                            "Switch Active Device",
                            Some(switch_badge),
                            Some(if is_active {
                                Color32::from_rgb(34, 197, 94)
                            } else {
                                accent
                            }),
                            false,
                            theme,
                        )
                        .clicked()
                        {
                            chosen_action = ContextMenuAction::SwitchButton(idx);
                            should_close = true;
                        }

                        if draw_context_action_row(
                            ui,
                            "Customize Button…",
                            Some("Edit"),
                            None,
                            false,
                            theme,
                        )
                        .clicked()
                        {
                            chosen_action = ContextMenuAction::EditButton(idx);
                            should_close = true;
                        }

                        if draw_context_action_row(
                            ui,
                            "Duplicate Button",
                            Some("Copy"),
                            None,
                            false,
                            theme,
                        )
                        .clicked()
                        {
                            chosen_action = ContextMenuAction::DuplicateButton(idx);
                            should_close = true;
                        }

                        if draw_context_action_row(
                            ui,
                            "Delete Button",
                            None,
                            None,
                            true,
                            theme,
                        )
                        .clicked()
                        {
                            chosen_action = ContextMenuAction::DeleteButton(idx);
                            should_close = true;
                        }
                    });

                    ui.add_space(6.0);
                }

                // ── Section 2: Add Button & Audio Routing ───────────────────────────
                group_frame.show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;

                    if draw_context_action_row(
                        ui,
                        "+ Add New Audio Button…",
                        Some("New"),
                        Some(default_accent),
                        false,
                        theme,
                    )
                    .clicked()
                    {
                        chosen_action = ContextMenuAction::AddButton;
                        should_close = true;
                    }

                    // Hairline divider
                    let (div_rect, _) = ui.allocate_exact_size(
                        Vec2::new(ui.available_width(), 5.0),
                        Sense::hover(),
                    );
                    ui.painter().line_segment(
                        [
                            Pos2::new(div_rect.left() + 4.0, div_rect.center().y),
                            Pos2::new(div_rect.right() - 4.0, div_rect.center().y),
                        ],
                        Stroke::new(1.0, divider_col),
                    );

                    if draw_context_toggle_row(
                        ui,
                        "Mute All Outputs",
                        outputs_muted,
                        Color32::from_rgb(239, 68, 68),
                        None,
                        theme,
                    )
                    .clicked()
                    {
                        chosen_action = ContextMenuAction::ToggleMuteOutputs;
                    }

                    if draw_context_toggle_row(
                        ui,
                        "Mute All Microphones",
                        inputs_muted,
                        Color32::from_rgb(239, 68, 68),
                        None,
                        theme,
                    )
                    .clicked()
                    {
                        chosen_action = ContextMenuAction::ToggleMuteInputs;
                    }
                });

                ui.add_space(6.0);

                // ── Section 3: Workspace & View Toggles ─────────────────────────────
                group_frame.show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;

                    if draw_context_toggle_row(
                        ui,
                        "Minimalist Frameless Mode",
                        minimalist_mode,
                        default_accent,
                        Some("F2"),
                        theme,
                    )
                    .clicked()
                    {
                        chosen_action = ContextMenuAction::ToggleMinimalist;
                    }

                    if draw_context_toggle_row(
                        ui,
                        "Dark Mode (Matte Obsidian)",
                        dark_mode,
                        default_accent,
                        None,
                        theme,
                    )
                    .clicked()
                    {
                        chosen_action = ContextMenuAction::ToggleTheme;
                    }

                    if draw_context_toggle_row(
                        ui,
                        "Show Text Labels",
                        labels_shown,
                        default_accent,
                        None,
                        theme,
                    )
                    .clicked()
                    {
                        chosen_action = ContextMenuAction::ToggleLabels;
                    }

                    if draw_context_toggle_row(
                        ui,
                        "Always on Top",
                        always_on_top,
                        default_accent,
                        None,
                        theme,
                    )
                    .clicked()
                    {
                        chosen_action = ContextMenuAction::ToggleAlwaysOnTop;
                    }
                });

                ui.add_space(6.0);

                // ── Section 4: Footer Actions (Hide to Tray / Quit Lyre) ─────────────
                ui.columns(2, |cols| {
                    if draw_context_footer_button(&mut cols[0], "Hide to Tray", false, theme)
                        .clicked()
                    {
                        chosen_action = ContextMenuAction::HideToTray;
                        should_close = true;
                    }
                    if draw_context_footer_button(&mut cols[1], "Quit Lyre", true, theme)
                        .clicked()
                    {
                        chosen_action = ContextMenuAction::QuitApp;
                        should_close = true;
                    }
                });
            });
    });

    if should_close {
        editor.context_menu = None;
    } else {
        menu_state.just_opened = false;
        editor.context_menu = Some(menu_state);
    }

    chosen_action
}

/// Draws a sleek **Linear / Raycast Matte Obsidian** action row inside the context window.
fn draw_context_action_row(
    ui: &mut Ui,
    label: &str,
    right_badge: Option<&str>,
    accent_dot: Option<Color32>,
    destructive: bool,
    theme: ThemeMode,
) -> egui::Response {
    let desired = Vec2::new(ui.available_width(), 27.0);
    let (rect, resp) = ui.allocate_exact_size(desired, Sense::click());
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }

    let (hover_bg, hover_border, text_col, badge_bg, badge_fg) = match theme {
        ThemeMode::Dark => {
            if destructive {
                (
                    Color32::from_rgba_unmultiplied(239, 68, 68, 30),
                    Color32::from_rgba_unmultiplied(239, 68, 68, 120),
                    Color32::from_rgb(248, 113, 113),
                    Color32::from_rgb(32, 36, 45),
                    Color32::from_rgb(156, 163, 175),
                )
            } else {
                (
                    Color32::from_rgb(30, 34, 43),
                    Color32::from_rgb(50, 56, 70),
                    if resp.hovered() {
                        Color32::WHITE
                    } else {
                        Color32::from_rgb(228, 231, 239)
                    },
                    Color32::from_rgb(29, 32, 40),
                    Color32::from_rgb(148, 155, 172),
                )
            }
        }
        ThemeMode::Light => {
            if destructive {
                (
                    Color32::from_rgba_unmultiplied(220, 38, 38, 22),
                    Color32::from_rgba_unmultiplied(220, 38, 38, 100),
                    Color32::from_rgb(210, 32, 32),
                    Color32::from_rgb(232, 235, 244),
                    Color32::from_rgb(95, 100, 114),
                )
            } else {
                (
                    Color32::from_rgb(235, 238, 246),
                    Color32::from_rgb(206, 212, 224),
                    Color32::from_rgb(22, 24, 30),
                    Color32::from_rgb(234, 237, 245),
                    Color32::from_rgb(95, 100, 114),
                )
            }
        }
    };

    if resp.hovered() {
        let rounding = Rounding::same(6.0);
        ui.painter().rect_filled(rect, rounding, hover_bg);
        ui.painter()
            .rect_stroke(rect, rounding, Stroke::new(1.0, hover_border));
    }

    let mut text_x = rect.left() + 8.0;
    if let Some(dot_color) = accent_dot {
        ui.painter().circle_filled(
            Pos2::new(rect.left() + 11.0, rect.center().y),
            3.5,
            dot_color,
        );
        text_x = rect.left() + 21.0;
    }

    ui.painter().text(
        Pos2::new(text_x, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::new(12.2, FontFamily::Proportional),
        text_col,
    );

    if let Some(badge) = right_badge {
        let font = FontId::new(10.0, FontFamily::Proportional);
        let galley = ui
            .painter()
            .layout_no_wrap(badge.to_string(), font.clone(), badge_fg);
        let bw = galley.size().x + 10.0;
        let badge_rect = Rect::from_center_size(
            Pos2::new(rect.right() - bw * 0.5 - 6.0, rect.center().y),
            Vec2::new(bw, 17.0),
        );
        ui.painter()
            .rect_filled(badge_rect, Rounding::same(4.0), badge_bg);
        ui.painter().text(
            badge_rect.center(),
            Align2::CENTER_CENTER,
            badge,
            font,
            badge_fg,
        );
    }

    resp
}

/// Draws a sleek **Linear / Raycast Matte Obsidian** toggle row with an inline pill switch.
fn draw_context_toggle_row(
    ui: &mut Ui,
    label: &str,
    active: bool,
    active_accent: Color32,
    shortcut_badge: Option<&str>,
    theme: ThemeMode,
) -> egui::Response {
    let desired = Vec2::new(ui.available_width(), 27.0);
    let (rect, resp) = ui.allocate_exact_size(desired, Sense::click());
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }

    let (hover_bg, hover_border, text_col, badge_bg, badge_fg) = match theme {
        ThemeMode::Dark => (
            Color32::from_rgb(30, 34, 43),
            Color32::from_rgb(50, 56, 70),
            if resp.hovered() || active {
                Color32::WHITE
            } else {
                Color32::from_rgb(218, 222, 232)
            },
            Color32::from_rgb(29, 32, 40),
            Color32::from_rgb(142, 148, 164),
        ),
        ThemeMode::Light => (
            Color32::from_rgb(235, 238, 246),
            Color32::from_rgb(206, 212, 224),
            Color32::from_rgb(22, 24, 30),
            Color32::from_rgb(234, 237, 245),
            Color32::from_rgb(95, 100, 114),
        ),
    };

    if resp.hovered() {
        let rounding = Rounding::same(6.0);
        ui.painter().rect_filled(rect, rounding, hover_bg);
        ui.painter()
            .rect_stroke(rect, rounding, Stroke::new(1.0, hover_border));
    }

    ui.painter().text(
        Pos2::new(rect.left() + 8.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::new(12.2, FontFamily::Proportional),
        text_col,
    );

    // Inline Linear pill toggle switch on the right
    let switch_rect = Rect::from_center_size(
        Pos2::new(rect.right() - 21.0, rect.center().y),
        Vec2::new(30.0, 16.0),
    );
    let (track_bg, track_border) = if active {
        (active_accent, active_accent)
    } else {
        match theme {
            ThemeMode::Dark => (
                Color32::from_rgb(30, 33, 41),
                Color32::from_rgb(52, 56, 68),
            ),
            ThemeMode::Light => (
                Color32::from_rgb(214, 218, 228),
                Color32::from_rgb(186, 192, 204),
            ),
        }
    };
    ui.painter()
        .rect_filled(switch_rect, Rounding::same(8.0), track_bg);
    ui.painter()
        .rect_stroke(switch_rect, Rounding::same(8.0), Stroke::new(1.0, track_border));

    let knob_x = if active {
        switch_rect.right() - 8.0
    } else {
        switch_rect.left() + 8.0
    };
    ui.painter().circle_filled(
        Pos2::new(knob_x, switch_rect.center().y),
        5.6,
        Color32::WHITE,
    );

    if let Some(badge) = shortcut_badge {
        let font = FontId::new(9.8, FontFamily::Proportional);
        let galley = ui
            .painter()
            .layout_no_wrap(badge.to_string(), font.clone(), badge_fg);
        let bw = galley.size().x + 9.0;
        let badge_rect = Rect::from_center_size(
            Pos2::new(switch_rect.left() - bw * 0.5 - 6.0, rect.center().y),
            Vec2::new(bw, 16.0),
        );
        ui.painter()
            .rect_filled(badge_rect, Rounding::same(4.0), badge_bg);
        ui.painter().text(
            badge_rect.center(),
            Align2::CENTER_CENTER,
            badge,
            font,
            badge_fg,
        );
    }

    resp
}

/// Draws a sleek footer pill button (`Hide to Tray` or `Quit Lyre`) at the bottom of the context window.
fn draw_context_footer_button(
    ui: &mut Ui,
    label: &str,
    destructive: bool,
    theme: ThemeMode,
) -> egui::Response {
    let desired = Vec2::new(ui.available_width(), 28.0);
    let (rect, resp) = ui.allocate_exact_size(desired, Sense::click());
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }

    let (bg, border, text_col) = match theme {
        ThemeMode::Dark => {
            if destructive {
                if resp.hovered() {
                    (
                        Color32::from_rgba_unmultiplied(239, 68, 68, 42),
                        Color32::from_rgb(239, 68, 68),
                        Color32::WHITE,
                    )
                } else {
                    (
                        Color32::from_rgba_unmultiplied(239, 68, 68, 18),
                        Color32::from_rgba_unmultiplied(239, 68, 68, 95),
                        Color32::from_rgb(248, 113, 113),
                    )
                }
            } else if resp.hovered() {
                (
                    Color32::from_rgb(30, 34, 43),
                    Color32::from_rgb(56, 62, 76),
                    Color32::WHITE,
                )
            } else {
                (
                    Color32::from_rgb(19, 21, 27),
                    Color32::from_rgb(36, 40, 50),
                    Color32::from_rgb(208, 213, 225),
                )
            }
        }
        ThemeMode::Light => {
            if destructive {
                (
                    Color32::from_rgba_unmultiplied(220, 38, 38, 18),
                    Color32::from_rgba_unmultiplied(220, 38, 38, 110),
                    Color32::from_rgb(200, 30, 30),
                )
            } else {
                (
                    Color32::from_rgb(255, 255, 255),
                    Color32::from_rgb(212, 216, 228),
                    Color32::from_rgb(36, 40, 50),
                )
            }
        }
    };

    let rounding = Rounding::same(7.0);
    ui.painter().rect_filled(rect, rounding, bg);
    ui.painter()
        .rect_stroke(rect, rounding, Stroke::new(1.0, border));
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::new(11.8, FontFamily::Proportional),
        text_col,
    );

    resp
}
