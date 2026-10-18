use egui::{Color32, FontData, FontDefinitions, FontFamily, Id, Rounding, Stroke, Style, Visuals};

use crate::config::{parse_hex_color, ThemeMode};

pub const BUTTON_FONT_FAMILY_NAME: &str = "bitcount_button";

/// Returns the custom `FontFamily` (`Bitcount Prop Single`) used for rendering audio button labels and badges.
/// Automatically ensures the custom font is registered and safely falls back to `Proportional` if called mid-frame before atlas rebuild.
pub fn button_font_family(ctx: &egui::Context) -> FontFamily {
    ensure_custom_fonts(ctx);
    let custom = FontFamily::Name(BUTTON_FONT_FAMILY_NAME.into());
    let is_bound = ctx.fonts(|f| f.families().contains(&custom));
    if is_bound {
        custom
    } else {
        FontFamily::Proportional
    }
}

/// Ensures `BitcountPropSingle-Medium.ttf` from `assets/fonts/Bitcount_Prop_Single` is registered
/// in the `egui::Context` under `button_font_family()` (with fallback to the default proportional font).
pub fn ensure_custom_fonts(ctx: &egui::Context) {
    let font_init_id = Id::new("lyre_bitcount_font_installed");
    let already_installed = ctx.data(|d| d.get_temp::<bool>(font_init_id).unwrap_or(false));
    if already_installed {
        return;
    }

    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "bitcount_prop_single".to_owned(),
        FontData::from_static(include_bytes!(
            "../../assets/fonts/Bitcount_Prop_Single/static/BitcountPropSingle-Medium.ttf"
        )),
    );

    let mut family_list = vec!["bitcount_prop_single".to_owned()];
    if let Some(default_prop) = fonts.families.get(&FontFamily::Proportional) {
        family_list.extend(default_prop.iter().cloned());
    }
    fonts
        .families
        .insert(FontFamily::Name(BUTTON_FONT_FAMILY_NAME.into()), family_list);

    ctx.set_fonts(fonts);
    ctx.data_mut(|d| d.insert_temp(font_init_id, true));
}

/// Resolved visual colors for rendering a single audio device button
/// in the **Linear / Raycast Matte Obsidian** design system.
#[derive(Debug, Clone, Copy)]
pub struct ButtonVisualPalette {
    pub body_fill: Color32,
    pub top_rim_highlight: Color32,
    pub border_color: Color32,
    pub border_width_mult: f32,
    pub glow_color: Option<Color32>,
    pub icon_well_fill: Color32,
    pub icon_well_stroke: Color32,
    pub icon_tint: Color32,
    pub text_color: Color32,
    pub subtext_color: Color32,
    pub badge_bg: Color32,
    pub badge_fg: Color32,
}

/// Applies global `egui` visuals for either Dark Mode (Linear / Raycast Matte Obsidian)
/// or Light Mode (Clean Alabaster Slate), and ensures the custom button font is loaded.
pub fn apply_theme(ctx: &egui::Context, mode: ThemeMode) {
    ensure_custom_fonts(ctx);
    let mut style: Style = (*ctx.style()).clone();
    let visuals = match mode {
        ThemeMode::Dark => {
            let mut v = Visuals::dark();
            v.dark_mode = true;
            // Linear / Raycast Matte Obsidian carbon surfaces
            v.panel_fill = Color32::from_rgb(13, 14, 17); // #0D0E11
            v.window_fill = Color32::from_rgb(19, 21, 25); // #131519
            v.extreme_bg_color = Color32::from_rgb(10, 11, 14); // #0A0B0E
            v.faint_bg_color = Color32::from_rgb(24, 26, 32);
            v.window_stroke = Stroke::new(1.0, Color32::from_rgb(38, 41, 50)); // #262932
            v.window_rounding = Rounding::same(10.0);
            v.menu_rounding = Rounding::same(8.0);

            v.widgets.noninteractive.bg_fill = Color32::from_rgb(20, 22, 27);
            v.widgets.noninteractive.weak_bg_fill = Color32::from_rgb(20, 22, 27);
            v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(36, 39, 48));
            v.widgets.noninteractive.fg_stroke =
                Stroke::new(1.0, Color32::from_rgb(228, 230, 238));

            v.widgets.inactive.bg_fill = Color32::from_rgb(25, 28, 34);
            v.widgets.inactive.weak_bg_fill = Color32::from_rgb(25, 28, 34);
            v.widgets.inactive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(44, 48, 58));
            v.widgets.inactive.fg_stroke = Stroke::new(1.0, Color32::from_rgb(232, 234, 242));
            v.widgets.inactive.rounding = Rounding::same(7.0);

            v.widgets.hovered.bg_fill = Color32::from_rgb(33, 37, 45);
            v.widgets.hovered.weak_bg_fill = Color32::from_rgb(33, 37, 45);
            v.widgets.hovered.bg_stroke = Stroke::new(1.0, Color32::from_rgb(70, 75, 90));
            v.widgets.hovered.fg_stroke = Stroke::new(1.0, Color32::WHITE);
            v.widgets.hovered.rounding = Rounding::same(7.0);

            v.widgets.active.bg_fill = Color32::from_rgb(42, 46, 56);
            v.widgets.active.weak_bg_fill = Color32::from_rgb(42, 46, 56);
            v.widgets.active.bg_stroke = Stroke::new(1.0, Color32::from_rgb(98, 104, 122));
            v.widgets.active.fg_stroke = Stroke::new(1.0, Color32::WHITE);
            v.widgets.active.rounding = Rounding::same(7.0);

            v.selection.bg_fill = Color32::from_rgb(52, 57, 70);
            v.selection.stroke = Stroke::new(1.0, Color32::WHITE);
            v
        }
        ThemeMode::Light => {
            let mut v = Visuals::light();
            v.dark_mode = false;
            v.panel_fill = Color32::from_rgb(244, 245, 248); // #F4F5F8
            v.window_fill = Color32::from_rgb(255, 255, 255);
            v.extreme_bg_color = Color32::from_rgb(236, 238, 243);
            v.faint_bg_color = Color32::from_rgb(248, 249, 251);
            v.window_stroke = Stroke::new(1.0, Color32::from_rgb(218, 221, 230));
            v.window_rounding = Rounding::same(10.0);
            v.menu_rounding = Rounding::same(8.0);

            v.widgets.inactive.bg_fill = Color32::from_rgb(236, 238, 244);
            v.widgets.inactive.weak_bg_fill = Color32::from_rgb(236, 238, 244);
            v.widgets.inactive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(210, 214, 224));
            v.widgets.inactive.fg_stroke = Stroke::new(1.0, Color32::from_rgb(24, 26, 32));
            v.widgets.inactive.rounding = Rounding::same(7.0);

            v.widgets.hovered.bg_fill = Color32::from_rgb(226, 229, 238);
            v.widgets.hovered.weak_bg_fill = Color32::from_rgb(226, 229, 238);
            v.widgets.hovered.rounding = Rounding::same(7.0);

            v.widgets.active.rounding = Rounding::same(7.0);
            v
        }
    };

    style.visuals = visuals;
    style.spacing.item_spacing = egui::vec2(6.0, 6.0);
    style.spacing.button_padding = egui::vec2(8.0, 4.0);
    ctx.set_style(style);
}

/// Computes the uniform matte surface, hairline border, and icon well colors for a button:
/// - Zero two-tone split bands; 100% uniform matte surface across the entire card.
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
            // Uniform matte obsidian card surface (#15171C idle, #1C1F26 hover, #111317 pressed)
            let base_matte = if is_pressed {
                Color32::from_rgb(17, 19, 23)
            } else if is_hovered {
                Color32::from_rgb(28, 31, 38)
            } else {
                Color32::from_rgb(21, 23, 28)
            };

            // Active state blends a uniform 10-14% accent tint across the entire matte card
            let body_fill = if is_active {
                blend_colors(base_matte, accent, if is_hovered { 0.14 } else { 0.10 })
            } else {
                base_matte
            };

            let top_rim_highlight = if is_active {
                Color32::from_rgba_unmultiplied(255, 255, 255, 28)
            } else if is_hovered {
                Color32::from_rgba_unmultiplied(255, 255, 255, 20)
            } else {
                Color32::from_rgba_unmultiplied(255, 255, 255, 12)
            };

            // Hairline 1px border: understated accent-blended slate when idle, crisp accent when active
            let border_color = if is_active {
                brighten_color(accent, 0.18)
            } else if is_hovered {
                blend_colors(Color32::from_rgb(58, 63, 76), accent, 0.68)
            } else {
                blend_colors(Color32::from_rgb(38, 41, 50), accent, 0.42)
            };

            let glow_color = if is_active {
                Some(Color32::from_rgba_unmultiplied(r, g, b, 36))
            } else {
                None
            };

            // Subtle Raycast/Linear squircle well behind preset icons
            let icon_well_fill = if is_active {
                Color32::from_rgba_unmultiplied(r, g, b, 42)
            } else if is_hovered {
                Color32::from_rgba_unmultiplied(r, g, b, 26)
            } else {
                Color32::from_rgba_unmultiplied(r, g, b, 16)
            };

            let icon_well_stroke = if is_active {
                Color32::from_rgba_unmultiplied(r, g, b, 110)
            } else if is_hovered {
                Color32::from_rgba_unmultiplied(r, g, b, 65)
            } else {
                Color32::from_rgba_unmultiplied(r, g, b, 36)
            };

            let icon_tint = if is_active {
                Color32::WHITE
            } else if is_hovered {
                brighten_color(accent, 0.58)
            } else {
                brighten_color(accent, 0.42)
            };

            let text_color = if is_active {
                Color32::from_rgb(250, 251, 254)
            } else if is_hovered {
                Color32::from_rgb(236, 238, 245)
            } else {
                Color32::from_rgb(206, 210, 222)
            };

            let subtext_color = Color32::from_rgba_unmultiplied(
                text_color.r(),
                text_color.g(),
                text_color.b(),
                115,
            );

            let badge_fg = contrasting_foreground(r, g, b);

            ButtonVisualPalette {
                body_fill,
                top_rim_highlight,
                border_color,
                border_width_mult: if is_active { 1.45 } else { 1.0 },
                glow_color,
                icon_well_fill,
                icon_well_stroke,
                icon_tint,
                text_color,
                subtext_color,
                badge_bg: accent,
                badge_fg,
            }
        }
        ThemeMode::Light => {
            // Light Mode: Clean uniform matte surface with subtle or rich user color fill
            let base_light = if is_pressed {
                blend_colors(Color32::from_rgb(245, 246, 250), accent, 0.24)
            } else if is_active {
                blend_colors(Color32::from_rgb(255, 255, 255), accent, 0.20)
            } else if is_hovered {
                blend_colors(Color32::from_rgb(255, 255, 255), accent, 0.12)
            } else {
                blend_colors(Color32::from_rgb(255, 255, 255), accent, 0.06)
            };

            let border_color = if is_active {
                darken_color(accent, 0.08)
            } else if is_hovered {
                blend_colors(Color32::from_rgb(195, 200, 212), accent, 0.65)
            } else {
                blend_colors(Color32::from_rgb(214, 218, 228), accent, 0.40)
            };

            let glow_color = if is_active {
                Some(Color32::from_rgba_unmultiplied(r, g, b, 34))
            } else {
                None
            };

            let icon_well_fill = if is_active {
                Color32::from_rgba_unmultiplied(r, g, b, 45)
            } else {
                Color32::from_rgba_unmultiplied(r, g, b, 24)
            };
            let icon_well_stroke = Color32::from_rgba_unmultiplied(r, g, b, 75);

            let icon_tint = darken_color(accent, 0.18);
            let text_color = Color32::from_rgb(22, 24, 30);
            let subtext_color = Color32::from_rgb(110, 115, 128);

            let badge_fg = contrasting_foreground(r, g, b);

            ButtonVisualPalette {
                body_fill: base_light,
                top_rim_highlight: Color32::from_rgba_unmultiplied(255, 255, 255, 180),
                border_color,
                border_width_mult: if is_active { 1.55 } else { 1.0 },
                glow_color,
                icon_well_fill,
                icon_well_stroke,
                icon_tint,
                text_color,
                subtext_color,
                badge_bg: accent,
                badge_fg,
            }
        }
    }
}

fn contrasting_foreground(r: u8, g: u8, b: u8) -> Color32 {
    let lum = (0.299 * (r as f32) + 0.587 * (g as f32) + 0.114 * (b as f32)) / 255.0;
    if lum > 0.58 {
        Color32::from_rgb(15, 17, 21)
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
