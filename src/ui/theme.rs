use egui::{Color32, Rounding, Stroke, Style, Visuals};

use crate::config::{parse_hex_color, ThemeMode};

/// Resolved visual colors for rendering a single audio device button.
#[derive(Debug, Clone, Copy)]
pub struct ButtonVisualPalette {
    pub body_fill: Color32,
    pub inner_sheen: Color32,
    pub border_color: Color32,
    pub border_width_mult: f32,
    pub glow_color: Option<Color32>,
    pub icon_tint: Color32,
    pub text_color: Color32,
    pub subtext_color: Color32,
    pub badge_bg: Color32,
    pub badge_fg: Color32,
    pub active_pill_color: Color32,
}

/// Applies global `egui` visuals for either Dark Mode or Light Mode.
/// Ensures all panels, modals, dropdowns, and buttons share a cohesive neutral palette.
pub fn apply_theme(ctx: &egui::Context, mode: ThemeMode) {
    let mut style: Style = (*ctx.style()).clone();
    let visuals = match mode {
        ThemeMode::Dark => {
            let mut v = Visuals::dark();
            v.dark_mode = true;
            // Neutral dark grey surfaces (no blue cast)
            v.panel_fill = Color32::from_rgb(17, 17, 19); // #111113
            v.window_fill = Color32::from_rgb(24, 24, 27); // #18181B
            v.extreme_bg_color = Color32::from_rgb(12, 12, 14);
            v.faint_bg_color = Color32::from_rgb(28, 28, 32);
            v.window_stroke = Stroke::new(1.0, Color32::from_rgb(46, 46, 52));
            v.window_rounding = Rounding::same(10.0);
            v.menu_rounding = Rounding::same(8.0);

            v.widgets.noninteractive.bg_fill = Color32::from_rgb(24, 24, 27);
            v.widgets.noninteractive.weak_bg_fill = Color32::from_rgb(24, 24, 27);
            v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(42, 42, 48));
            v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, Color32::from_rgb(228, 228, 232));

            v.widgets.inactive.bg_fill = Color32::from_rgb(34, 34, 38);
            v.widgets.inactive.weak_bg_fill = Color32::from_rgb(32, 32, 36);
            v.widgets.inactive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(52, 52, 58));
            v.widgets.inactive.fg_stroke = Stroke::new(1.0, Color32::from_rgb(232, 232, 236));
            v.widgets.inactive.rounding = Rounding::same(6.0);

            v.widgets.hovered.bg_fill = Color32::from_rgb(46, 46, 52);
            v.widgets.hovered.weak_bg_fill = Color32::from_rgb(44, 44, 50);
            v.widgets.hovered.bg_stroke = Stroke::new(1.0, Color32::from_rgb(82, 82, 91));
            v.widgets.hovered.fg_stroke = Stroke::new(1.0, Color32::WHITE);
            v.widgets.hovered.rounding = Rounding::same(6.0);

            v.widgets.active.bg_fill = Color32::from_rgb(58, 58, 66);
            v.widgets.active.weak_bg_fill = Color32::from_rgb(56, 56, 64);
            v.widgets.active.bg_stroke = Stroke::new(1.0, Color32::from_rgb(113, 113, 122));
            v.widgets.active.fg_stroke = Stroke::new(1.0, Color32::WHITE);
            v.widgets.active.rounding = Rounding::same(6.0);

            v.selection.bg_fill = Color32::from_rgb(63, 63, 70);
            v.selection.stroke = Stroke::new(1.0, Color32::WHITE);
            v
        }
        ThemeMode::Light => {
            let mut v = Visuals::light();
            v.dark_mode = false;
            v.panel_fill = Color32::from_rgb(244, 244, 246);
            v.window_fill = Color32::from_rgb(252, 252, 253);
            v.window_stroke = Stroke::new(1.0, Color32::from_rgb(218, 218, 224));
            v.window_rounding = Rounding::same(10.0);
            v.menu_rounding = Rounding::same(8.0);

            v.widgets.inactive.bg_fill = Color32::from_rgb(232, 232, 236);
            v.widgets.inactive.weak_bg_fill = Color32::from_rgb(232, 232, 236);
            v.widgets.inactive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(208, 208, 214));
            v.widgets.inactive.fg_stroke = Stroke::new(1.0, Color32::from_rgb(28, 28, 32));
            v.widgets.inactive.rounding = Rounding::same(6.0);

            v.widgets.hovered.bg_fill = Color32::from_rgb(220, 220, 226);
            v.widgets.hovered.weak_bg_fill = Color32::from_rgb(220, 220, 226);
            v.widgets.hovered.rounding = Rounding::same(6.0);

            v.widgets.active.rounding = Rounding::same(6.0);
            v
        }
    };

    style.visuals = visuals;
    style.spacing.item_spacing = egui::vec2(6.0, 6.0);
    style.spacing.button_padding = egui::vec2(8.0, 4.0);
    ctx.set_style(style);
}

/// Computes the exact colors for a button based on:
/// - `theme_mode` (`Light` vs `Dark`)
/// - `color_hex` (user-selected color)
/// - `is_active` (whether this endpoint is the active Default Output or Default Input)
/// - `is_hovered` / `is_pressed`
pub fn resolve_button_palette(
    theme_mode: ThemeMode,
    color_hex: &str,
    is_active: bool,
    is_hovered: bool,
    is_pressed: bool,
) -> ButtonVisualPalette {
    let [r, g, b] = parse_hex_color(color_hex).unwrap_or([0x3B, 0x82, 0xF6]);
    let accent = Color32::from_rgb(r, g, b);

    match theme_mode {
        ThemeMode::Dark => {
            // Neutral Dark Grey card body (#1A1A1D inactive, #242428 hovered)
            let base_dark = if is_pressed {
                Color32::from_rgb(20, 20, 22)
            } else if is_hovered {
                Color32::from_rgb(35, 35, 39)
            } else {
                Color32::from_rgb(26, 26, 29)
            };

            let body_fill = if is_active {
                blend_colors(base_dark, accent, if is_hovered { 0.22 } else { 0.14 })
            } else {
                base_dark
            };

            let inner_sheen = if is_active {
                Color32::from_rgba_unmultiplied(r, g, b, 28)
            } else if is_hovered {
                Color32::from_rgba_unmultiplied(255, 255, 255, 10)
            } else {
                Color32::from_rgba_unmultiplied(255, 255, 255, 5)
            };

            let border_color = if is_active {
                brighten_color(accent, 0.22)
            } else if is_hovered {
                brighten_color(accent, 0.05)
            } else {
                Color32::from_rgba_unmultiplied(r, g, b, 155)
            };

            let glow_color = if is_active {
                Some(Color32::from_rgba_unmultiplied(r, g, b, 65))
            } else if is_hovered {
                Some(Color32::from_rgba_unmultiplied(r, g, b, 24))
            } else {
                None
            };

            let icon_tint = if is_active {
                Color32::WHITE
            } else if is_hovered {
                brighten_color(accent, 0.55)
            } else {
                brighten_color(accent, 0.35)
            };

            let text_color = if is_active {
                Color32::WHITE
            } else if is_hovered {
                Color32::from_rgb(242, 242, 245)
            } else {
                Color32::from_rgb(212, 212, 218)
            };

            let subtext_color = Color32::from_rgba_unmultiplied(
                text_color.r(),
                text_color.g(),
                text_color.b(),
                125,
            );

            let badge_fg = contrasting_foreground(r, g, b);

            ButtonVisualPalette {
                body_fill,
                inner_sheen,
                border_color,
                border_width_mult: if is_active { 2.0 } else { 1.15 },
                glow_color,
                icon_tint,
                text_color,
                subtext_color,
                badge_bg: accent,
                badge_fg,
                active_pill_color: brighten_color(accent, 0.25),
            }
        }
        ThemeMode::Light => {
            // Light Mode: User-selectable button fill color
            let body_fill = if is_pressed {
                darken_color(accent, 0.12)
            } else if is_hovered {
                brighten_color(accent, 0.06)
            } else if is_active {
                accent
            } else {
                blend_colors(Color32::from_rgb(255, 255, 255), accent, 0.84)
            };

            let [br, bg, bb, _] = body_fill.to_array();
            let fg = contrasting_foreground(br, bg, bb);

            let inner_sheen = Color32::from_rgba_unmultiplied(255, 255, 255, 36);

            let border_color = if is_active {
                Color32::from_rgb(24, 24, 27)
            } else if is_hovered {
                darken_color(accent, 0.24)
            } else {
                darken_color(accent, 0.14)
            };

            let glow_color = if is_active {
                Some(Color32::from_rgba_unmultiplied(r, g, b, 85))
            } else {
                None
            };

            let badge_bg = if fg == Color32::WHITE {
                Color32::from_rgb(24, 24, 27)
            } else {
                Color32::WHITE
            };
            let badge_fg = if badge_bg == Color32::WHITE {
                Color32::from_rgb(24, 24, 27)
            } else {
                Color32::WHITE
            };

            let subtext_color = Color32::from_rgba_unmultiplied(fg.r(), fg.g(), fg.b(), 145);

            ButtonVisualPalette {
                body_fill,
                inner_sheen,
                border_color,
                border_width_mult: if is_active { 2.3 } else { 1.0 },
                glow_color,
                icon_tint: fg,
                text_color: fg,
                subtext_color,
                badge_bg,
                badge_fg,
                active_pill_color: fg,
            }
        }
    }
}

fn contrasting_foreground(r: u8, g: u8, b: u8) -> Color32 {
    let lum = (0.299 * (r as f32) + 0.587 * (g as f32) + 0.114 * (b as f32)) / 255.0;
    if lum > 0.58 {
        Color32::from_rgb(20, 20, 23)
    } else {
        Color32::WHITE
    }
}

pub fn blend_colors(base: Color32, tint: Color32, factor: f32) -> Color32 {
    let f = factor.clamp(0.0, 1.0);
    let r = (base.r() as f32 * (1.0 - f) + tint.r() as f32 * f).round() as u8;
    let g = (base.g() as f32 * (1.0 - f) + tint.g() as f32 * f).round() as u8;
    let b = (base.b() as f32 * (1.0 - f) + tint.b() as f32 * f).round() as u8;
    Color32::from_rgb(r, g, b)
}

pub fn brighten_color(c: Color32, factor: f32) -> Color32 {
    blend_colors(c, Color32::WHITE, factor)
}

pub fn darken_color(c: Color32, factor: f32) -> Color32 {
    blend_colors(c, Color32::BLACK, factor)
}
