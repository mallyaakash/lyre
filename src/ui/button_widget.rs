use egui::{
    Align2, Color32, CursorIcon, FontId, Pos2, Rect, Rounding, Sense, Stroke, Ui, Vec2,
};

use crate::audio::EndpointDirection;
use crate::config::{parse_hex_color, ButtonConfig, IconSource, ThemeMode};
use crate::ui::icons::IconCache;
use crate::ui::theme::{button_font_family, resolve_button_palette};

/// Transient pop-up overlay shown briefly on a button card while scrolling volume or middle-clicking mute.
#[derive(Debug, Clone, Copy)]
pub struct ButtonVolumePopup {
    /// Current volume percentage (`0..=100`).
    pub volume_pct: u32,
    /// Whether the endpoint is currently muted.
    pub is_muted: bool,
    /// Animated opacity (`0.0..=1.0`) for smooth fade-out.
    pub opacity: f32,
}

/// Computed proportional metrics for a button of cell size `(width, height)`.
#[derive(Debug, Clone, Copy)]
pub struct ProportionalButtonMetrics {
    pub unit_l: f32,
    pub scale_s: f32,
    pub corner_radius: f32,
    pub base_stroke_width: f32,
    pub icon_size: f32,
    pub icon_center_offset_y: f32,
    pub badge_radius: f32,
    pub badge_offset: Vec2,
    pub badge_font_size: f32,
    pub label_font_size: f32,
    pub label_offset_y: f32,
}

impl ProportionalButtonMetrics {
    pub const REF_SIZE: f32 = 96.0;

    pub fn compute(cell_width: f32, cell_height: f32, show_label: bool) -> Self {
        let unit_l = cell_width.min(cell_height).max(20.0);
        let scale_s = unit_l / Self::REF_SIZE;
        let corner_radius = (0.14 * unit_l).clamp(4.0, 22.0);
        let base_stroke_width = (1.15 * scale_s).clamp(1.0, 3.0);

        // Button image size increased by 50% (0.39 * 1.5 = 0.585 when label shown; 0.72 when icon-only)
        let (icon_size, icon_center_offset_y) = if show_label {
            (0.585 * unit_l, -0.025 * unit_l)
        } else {
            (0.72 * unit_l, 0.03 * unit_l)
        };

        let badge_radius = (0.135 * icon_size).max(5.5);
        let badge_offset = Vec2::new(0.35 * icon_size, -0.33 * icon_size);
        let badge_font_size = (1.20 * badge_radius).clamp(6.8, 30.0);

        // Reduced text font size and lowered vertical offset so it fits cleanly beneath the 50% larger icon
        let label_font_size = (0.095 * unit_l).clamp(6.5, 18.5);
        let label_offset_y = 0.37 * unit_l;

        Self {
            unit_l,
            scale_s,
            corner_radius,
            base_stroke_width,
            icon_size,
            icon_center_offset_y,
            badge_radius,
            badge_offset,
            badge_font_size,
            label_font_size,
            label_offset_y,
        }
    }
}

/// Computes the hit-box `Rect` for the top-right three-vertical-ellipses (`⋮`) edit button.
pub fn compute_ellipsis_menu_rect(cell_rect: Rect, scale_s: f32) -> Rect {
    let menu_w = (20.0 * scale_s).clamp(16.0, 28.0);
    let menu_h = (24.0 * scale_s).clamp(18.0, 32.0);
    let menu_margin = (4.0 * scale_s).clamp(3.0, 7.0);
    Rect::from_min_size(
        Pos2::new(
            cell_rect.right() - menu_w - menu_margin,
            cell_rect.top() + menu_margin,
        ),
        Vec2::new(menu_w, menu_h),
    )
}

/// Renders a single proportionally scaled audio device button inside `cell_rect`
/// using the **Linear / Raycast Matte Obsidian** aesthetic (uniform matte carbon card,
/// subtle animated Studio VU background meter, crisp 1px hairline border, subtle icon well,
/// flush studio rectangular LED indicator, and brief scroll-wheel volume popup HUD).
#[allow(clippy::too_many_arguments)]
pub fn draw_audio_button(
    ui: &mut Ui,
    cell_rect: Rect,
    btn: &ButtonConfig,
    is_active: bool,
    is_muted: bool,
    is_device_present: bool,
    is_pressed: bool,
    is_being_dragged: bool,
    vu_level: f32,
    volume_popup: Option<ButtonVolumePopup>,
    theme_mode: ThemeMode,
    edit_mode: bool,
    icon_cache: &mut IconCache,
) {
    let id = ui.make_persistent_id(&btn.id);
    let _response = ui.interact(cell_rect, id, Sense::hover());

    let metrics =
        ProportionalButtonMetrics::compute(cell_rect.width(), cell_rect.height(), btn.show_label);
    let menu_rect = compute_ellipsis_menu_rect(cell_rect, metrics.scale_s);

    let pointer_pos = ui.input(|i| i.pointer.hover_pos().or_else(|| i.pointer.interact_pos()));
    let is_hovered = pointer_pos.map(|p| cell_rect.contains(p)).unwrap_or(false) || is_being_dragged;
    let menu_hovered = pointer_pos.map(|p| menu_rect.contains(p)).unwrap_or(false);

    if is_being_dragged {
        ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
    } else if is_hovered {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }

    let palette = resolve_button_palette(
        theme_mode,
        &btn.color_hex,
        is_active,
        is_hovered,
        is_pressed && !menu_hovered,
    );

    let draw_rect = if is_being_dragged {
        cell_rect
    } else if is_pressed && !menu_hovered {
        cell_rect.shrink(1.0)
    } else {
        cell_rect
    };

    let painter = ui.painter_at(cell_rect.expand(10.0 * metrics.scale_s));
    let rounding = Rounding::same(metrics.corner_radius);

    // 1. Subtle soft elevation shadow & optional active halo
    let shadow_offset = if is_being_dragged {
        Vec2::new(0.0, (4.5 * metrics.scale_s).clamp(3.0, 8.0))
    } else {
        Vec2::new(0.0, (1.5 * metrics.scale_s).clamp(1.0, 3.0))
    };
    let shadow_alpha = if is_being_dragged { 145 } else { 90 };
    painter.rect_filled(
        draw_rect.translate(shadow_offset),
        rounding,
        match theme_mode {
            ThemeMode::Dark => Color32::from_rgba_unmultiplied(0, 0, 0, shadow_alpha),
            ThemeMode::Light => Color32::from_rgba_unmultiplied(15, 23, 42, shadow_alpha / 4),
        },
    );

    if let Some(glow) = palette.glow_color {
        let glow_expand = (2.0 * metrics.scale_s).clamp(1.2, 4.5);
        painter.rect_filled(
            draw_rect.expand(glow_expand),
            Rounding::same(metrics.corner_radius + glow_expand * 0.5),
            glow,
        );
    }

    // 2. Uniform Matte Card Fill (Zero two-tone split bands)
    let body_fill = if is_device_present {
        palette.body_fill
    } else {
        Color32::from_rgba_unmultiplied(
            palette.body_fill.r(),
            palette.body_fill.g(),
            palette.body_fill.b(),
            135,
        )
    };
    painter.rect_filled(draw_rect, rounding, body_fill);

    // 2b. Subtle, Silky-Smooth Animated Studio VU Meter on the Background of the Button Card
    if vu_level > 0.004 && !is_muted {
        let [ar, ag, ab] = parse_hex_color(&btn.color_hex).unwrap_or([59, 130, 246]);
        draw_studio_vu_background(
            &painter,
            draw_rect,
            rounding,
            metrics.corner_radius,
            vu_level.clamp(0.0, 1.0),
            [ar, ag, ab],
            is_active,
            theme_mode,
        );
    }

    // 3. Crisp 1px Hairline Border + Single Top-Edge Rim Highlight
    let stroke_width = if is_being_dragged {
        (metrics.base_stroke_width * 2.0).max(1.8)
    } else {
        metrics.base_stroke_width * palette.border_width_mult
    };
    painter.rect_stroke(
        draw_rect,
        rounding,
        Stroke::new(stroke_width, palette.border_color),
    );

    let rim_inset = (metrics.corner_radius * 0.85).max(5.0);
    if draw_rect.width() > rim_inset * 2.2 {
        let rim_y = draw_rect.top() + 1.0;
        painter.line_segment(
            [
                Pos2::new(draw_rect.left() + rim_inset, rim_y),
                Pos2::new(draw_rect.right() - rim_inset, rim_y),
            ],
            Stroke::new(1.0, palette.top_rim_highlight),
        );
    }

    // 4. Studio Equipment Rectangular LED Indicator (Top-Center)
    //    - Muted  -> Lit Red studio rectangular light
    //    - Active -> Lit Green studio rectangular light
    //    - Unused -> Recessed Matte Grey rectangular slot
    draw_studio_led_indicator(
        &painter,
        draw_rect,
        metrics.scale_s,
        is_active,
        is_muted,
        theme_mode,
    );

    // 5. Centered Device Icon + Subtle Raycast/Linear Squircle Icon Well
    let icon_center = Pos2::new(
        draw_rect.center().x,
        draw_rect.center().y + metrics.icon_center_offset_y,
    );

    if matches!(btn.icon, IconSource::Preset(_)) {
        let max_well_h = if btn.show_label {
            draw_rect.height() * 0.56
        } else {
            draw_rect.height() * 0.72
        };
        let well_size = (metrics.icon_size * 0.98).min(max_well_h).min(draw_rect.width() * 0.76);
        let well_rect = Rect::from_center_size(icon_center, Vec2::splat(well_size));
        let well_rounding = Rounding::same((well_size * 0.24).clamp(4.0, 14.0));
        painter.rect_filled(well_rect, well_rounding, palette.icon_well_fill);
        painter.rect_stroke(
            well_rect,
            well_rounding,
            Stroke::new(1.0, palette.icon_well_stroke),
        );
    }

    let max_icon_bound = if btn.show_label {
        draw_rect.height() * 0.58
    } else {
        draw_rect.height() * 0.74
    };
    let icon_draw_size = match btn.icon {
        IconSource::Preset(_) => (metrics.icon_size * 0.84).min(max_icon_bound * 0.92),
        IconSource::CustomPath(_) => (metrics.icon_size * 1.06).min(max_icon_bound),
    };
    let icon_rect = Rect::from_center_size(icon_center, Vec2::splat(icon_draw_size));

    if let Some(tex) = icon_cache.get_texture(ui.ctx(), &btn.icon, icon_draw_size * 1.5) {
        let tint = match btn.icon {
            IconSource::Preset(_) => palette.icon_tint,
            // Preserve true colors and alpha transparency for uploaded PNG/WebP/SVG/ICO/JPG
            IconSource::CustomPath(_) => Color32::WHITE,
        };
        painter.image(
            tex.id(),
            icon_rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            tint,
        );
    } else {
        painter.circle_stroke(
            icon_center,
            metrics.icon_size * 0.35,
            Stroke::new(2.0 * metrics.scale_s, palette.icon_tint),
        );
    }

    let btn_font = button_font_family(ui.ctx());

    // 6. Optional Circular Corner Number Badge (1, 2, 3...) overlapping top-right of the icon well
    if btn.badge_enabled {
        let badge_center = icon_center + metrics.badge_offset;
        painter.circle_filled(
            badge_center,
            metrics.badge_radius + (1.5 * metrics.scale_s).max(1.2),
            palette.body_fill,
        );
        painter.circle_filled(badge_center, metrics.badge_radius, palette.badge_bg);
        painter.text(
            badge_center,
            Align2::CENTER_CENTER,
            btn.badge_number.to_string(),
            FontId::new(metrics.badge_font_size, btn_font.clone()),
            palette.badge_fg,
        );
    }

    // 7. Subtle Input ("MIC") / Output ("OUT") micro-tag in top-left corner
    if btn.direction == EndpointDirection::Input || edit_mode {
        let tag_font = (0.078 * metrics.unit_l).clamp(6.0, 10.0);
        let tag_text = match btn.direction {
            EndpointDirection::Output => "OUT",
            EndpointDirection::Input => "MIC",
        };
        let tag_pos = Pos2::new(
            draw_rect.left() + (8.0 * metrics.scale_s).max(5.5),
            draw_rect.top() + (6.5 * metrics.scale_s).max(5.0),
        );
        painter.text(
            tag_pos,
            Align2::LEFT_TOP,
            tag_text,
            FontId::new(tag_font, btn_font.clone()),
            palette.subtext_color,
        );
    }

    // 8. Optional Text Label ("Icon + Text Title" vs "Icon-Only") rendered in Bitcount Prop Single
    if btn.show_label && !btn.title.is_empty() {
        let label_pos = Pos2::new(
            draw_rect.center().x,
            draw_rect.center().y + metrics.label_offset_y,
        );
        let max_chars = ((draw_rect.width() * 0.90) / (metrics.label_font_size * 0.56))
            .floor()
            .max(4.0) as usize;
        let display_title = if btn.title.chars().count() > max_chars {
            let truncated: String = btn.title.chars().take(max_chars.saturating_sub(1)).collect();
            format!("{}…", truncated)
        } else {
            btn.title.clone()
        };

        painter.text(
            label_pos,
            Align2::CENTER_CENTER,
            display_title,
            FontId::new(metrics.label_font_size, btn_font.clone()),
            palette.text_color,
        );
    }

    // 9. Three Vertical Ellipses (`⋮`) Edit Button in the Top-Right Corner
    //    Always visible in Edit Mode, and visible on hover in Minimalist Mode.
    if edit_mode || is_hovered {
        if menu_hovered {
            let hover_pill = match theme_mode {
                ThemeMode::Dark => Color32::from_rgba_unmultiplied(255, 255, 255, 32),
                ThemeMode::Light => Color32::from_rgba_unmultiplied(0, 0, 0, 22),
            };
            painter.rect_filled(menu_rect, Rounding::same(menu_rect.width() * 0.35), hover_pill);
        }

        let dot_r = (1.45 * metrics.scale_s).clamp(1.2, 2.4);
        let dot_spacing = (4.3 * metrics.scale_s).clamp(3.3, 6.8);
        let dot_alpha = if menu_hovered {
            255
        } else if is_hovered {
            205
        } else {
            135
        };
        let dot_color = Color32::from_rgba_unmultiplied(
            palette.text_color.r(),
            palette.text_color.g(),
            palette.text_color.b(),
            dot_alpha,
        );

        let cx = menu_rect.center().x;
        let cy = menu_rect.center().y;
        painter.circle_filled(Pos2::new(cx, cy - dot_spacing), dot_r, dot_color);
        painter.circle_filled(Pos2::new(cx, cy), dot_r, dot_color);
        painter.circle_filled(Pos2::new(cx, cy + dot_spacing), dot_r, dot_color);
    }

    // 10. Brief Scroll-Wheel Volume / Middle-Click Mute HUD Pop-Up Overlay
    if let Some(popup) = volume_popup {
        if popup.opacity > 0.01 {
            let [ar, ag, ab] = parse_hex_color(&btn.color_hex).unwrap_or([59, 130, 246]);
            draw_volume_popup_overlay(
                &painter,
                draw_rect,
                metrics.scale_s,
                popup,
                [ar, ag, ab],
                &btn_font,
                theme_mode,
            );
        }
    }
}

/// Renders a subtle, silky-smooth animated Studio VU meter on the background of the button card.
/// Uses a multi-band vertical gradient rising from the bottom of the card with a delicate illuminated crest line.
#[allow(clippy::too_many_arguments)]
fn draw_studio_vu_background(
    painter: &egui::Painter,
    draw_rect: Rect,
    _rounding: Rounding,
    corner_radius: f32,
    vu_level: f32,
    accent_rgb: [u8; 3],
    is_active: bool,
    theme_mode: ThemeMode,
) {
    let inner = draw_rect.shrink(1.2);
    if inner.width() <= 4.0 || inner.height() <= 4.0 {
        return;
    }

    let max_vu_h = inner.height() * 0.88;
    let fill_h = (max_vu_h * vu_level).clamp(1.5, max_vu_h);
    let top_y = inner.bottom() - fill_h;

    // Blend the button's custom accent color with a touch of studio emerald when active
    let [r, g, b] = if is_active {
        [
            ((accent_rgb[0] as u16 * 3 + 34) / 4) as u8,
            ((accent_rgb[1] as u16 * 3 + 197) / 4) as u8,
            ((accent_rgb[2] as u16 * 3 + 94) / 4) as u8,
        ]
    } else {
        accent_rgb
    };

    let clipped = painter.with_clip_rect(inner);
    let bands = 10usize;
    let band_h = fill_h / (bands as f32);

    for i in 0..bands {
        // i = 0 is bottom band, i = bands - 1 is top crest band
        let t = (i as f32 + 0.5) / (bands as f32);
        let base_alpha = match theme_mode {
            ThemeMode::Dark => 10.0 + t * 28.0 * (0.55 + 0.45 * vu_level),
            ThemeMode::Light => 14.0 + t * 34.0 * (0.55 + 0.45 * vu_level),
        };
        let alpha = (base_alpha.round() as u8).clamp(6, 54);
        let y_max = inner.bottom() - (i as f32) * band_h;
        let y_min = (y_max - band_h - 0.6).max(top_y);
        let band_rect = Rect::from_min_max(
            Pos2::new(inner.left(), y_min),
            Pos2::new(inner.right(), y_max),
        );
        let band_rounding = if i == 0 {
            Rounding {
                nw: 0.0,
                ne: 0.0,
                sw: (corner_radius - 1.2).max(2.0),
                se: (corner_radius - 1.2).max(2.0),
            }
        } else {
            Rounding::ZERO
        };
        clipped.rect_filled(
            band_rect,
            band_rounding,
            Color32::from_rgba_unmultiplied(r, g, b, alpha),
        );
    }

    // Soft illuminated studio VU crest horizon line at top_y
    let crest_inset = if top_y > inner.bottom() - corner_radius {
        (corner_radius * 0.55).max(3.0)
    } else {
        2.0
    };
    if inner.width() > crest_inset * 2.5 {
        let crest_alpha = ((42.0 + 56.0 * vu_level).round() as u8).clamp(32, 105);
        clipped.line_segment(
            [
                Pos2::new(inner.left() + crest_inset, top_y),
                Pos2::new(inner.right() - crest_inset, top_y),
            ],
            Stroke::new(1.15, Color32::from_rgba_unmultiplied(r, g, b, crest_alpha)),
        );
    }
}

/// Renders a brief, auto-fading **Linear / Raycast Matte Obsidian** volume/mute HUD pill
/// over the lower portion of the button card when scrolling the wheel or middle-clicking.
fn draw_volume_popup_overlay(
    painter: &egui::Painter,
    draw_rect: Rect,
    scale_s: f32,
    popup: ButtonVolumePopup,
    accent_rgb: [u8; 3],
    btn_font: &egui::FontFamily,
    theme_mode: ThemeMode,
) {
    let alpha_f = popup.opacity.clamp(0.0, 1.0);
    let scale_alpha = |a: u8| -> u8 { ((a as f32) * alpha_f).round().clamp(0.0, 255.0) as u8 };

    let pill_w = (draw_rect.width() * 0.84).clamp(48.0, 148.0);
    let pill_h = (24.0 * scale_s).clamp(19.0, 30.0);
    let bottom_margin = (6.5 * scale_s).clamp(4.5, 10.0);
    let pill_rect = Rect::from_center_size(
        Pos2::new(
            draw_rect.center().x,
            draw_rect.bottom() - bottom_margin - pill_h * 0.5,
        ),
        Vec2::new(pill_w, pill_h),
    );
    let pill_rounding = Rounding::same((6.0 * scale_s).clamp(4.5, 8.0));

    let [ar, ag, ab] = if popup.is_muted {
        [239, 68, 68]
    } else {
        accent_rgb
    };

    let (bg_col, border_col, text_col, track_bg) = match theme_mode {
        ThemeMode::Dark => (
            Color32::from_rgba_unmultiplied(11, 12, 16, scale_alpha(238)),
            Color32::from_rgba_unmultiplied(ar, ag, ab, scale_alpha(195)),
            if popup.is_muted {
                Color32::from_rgba_unmultiplied(252, 165, 165, scale_alpha(255))
            } else {
                Color32::from_rgba_unmultiplied(248, 250, 252, scale_alpha(255))
            },
            Color32::from_rgba_unmultiplied(34, 38, 48, scale_alpha(230)),
        ),
        ThemeMode::Light => (
            Color32::from_rgba_unmultiplied(250, 251, 254, scale_alpha(244)),
            Color32::from_rgba_unmultiplied(ar, ag, ab, scale_alpha(185)),
            if popup.is_muted {
                Color32::from_rgba_unmultiplied(220, 38, 38, scale_alpha(255))
            } else {
                Color32::from_rgba_unmultiplied(15, 23, 42, scale_alpha(255))
            },
            Color32::from_rgba_unmultiplied(214, 218, 228, scale_alpha(230)),
        ),
    };

    painter.rect_filled(pill_rect, pill_rounding, bg_col);
    painter.rect_stroke(pill_rect, pill_rounding, Stroke::new(1.0, border_col));

    let label_text = if popup.is_muted {
        "MUTED".to_string()
    } else {
        format!("VOL {}%", popup.volume_pct.min(100))
    };
    let font_sz = (10.2 * scale_s).clamp(8.0, 12.8);
    painter.text(
        Pos2::new(pill_rect.center().x, pill_rect.top() + pill_h * 0.36),
        Align2::CENTER_CENTER,
        label_text,
        FontId::new(font_sz, btn_font.clone()),
        text_col,
    );

    // Mini horizontal studio progress bar inside the bottom of the HUD pill
    let bar_pad_x = (7.0 * scale_s).clamp(5.0, 10.0);
    let bar_h = (3.2 * scale_s).clamp(2.4, 4.2);
    let bar_y = pill_rect.bottom() - (4.2 * scale_s).clamp(3.2, 5.6);
    let track_rect = Rect::from_min_max(
        Pos2::new(pill_rect.left() + bar_pad_x, bar_y - bar_h),
        Pos2::new(pill_rect.right() - bar_pad_x, bar_y),
    );
    let bar_rounding = Rounding::same(bar_h * 0.5);
    painter.rect_filled(track_rect, bar_rounding, track_bg);

    let fill_ratio = if popup.is_muted {
        0.0
    } else {
        (popup.volume_pct.min(100) as f32) / 100.0
    };
    if fill_ratio > 0.0 {
        let fill_w = (track_rect.width() * fill_ratio).max(bar_h);
        let fill_rect = Rect::from_min_size(track_rect.min, Vec2::new(fill_w, track_rect.height()));
        painter.rect_filled(
            fill_rect,
            bar_rounding,
            Color32::from_rgba_unmultiplied(ar, ag, ab, scale_alpha(240)),
        );
    }
}

/// Renders a recessed **Linear / Raycast Matte Obsidian** drop-target slot in the grid
/// while a button tile is actively being dragged to reorder.
pub fn draw_drop_target_slot(
    ui: &mut Ui,
    cell_rect: Rect,
    accent_hex: &str,
    theme_mode: ThemeMode,
) {
    let metrics = ProportionalButtonMetrics::compute(cell_rect.width(), cell_rect.height(), true);
    let slot_rect = cell_rect.shrink(2.0);
    let rounding = Rounding::same(metrics.corner_radius);
    let [ar, ag, ab] = parse_hex_color(accent_hex).unwrap_or([59, 130, 246]);

    let (slot_fill, slot_border) = match theme_mode {
        ThemeMode::Dark => (
            Color32::from_rgba_unmultiplied(ar, ag, ab, 18),
            Color32::from_rgba_unmultiplied(ar, ag, ab, 150),
        ),
        ThemeMode::Light => (
            Color32::from_rgba_unmultiplied(ar, ag, ab, 24),
            Color32::from_rgba_unmultiplied(ar, ag, ab, 165),
        ),
    };

    let painter = ui.painter_at(cell_rect.expand(4.0));
    painter.rect_filled(slot_rect, rounding, slot_fill);
    painter.rect_stroke(slot_rect, rounding, Stroke::new(1.5, slot_border));
}

/// Draws a flush, modern studio-equipment rectangular LED indicator near the top-center of a button:
/// - **Muted (`is_muted`):** Uniform Crimson Red rectangular studio LED (`#EF4444`) with soft ambient halo.
/// - **Active (`is_active && !is_muted`):** Uniform Emerald Green rectangular studio LED (`#22C55E`) with soft ambient halo.
/// - **Unused (`!is_active && !is_muted`):** Recessed Matte Grey rectangular slot (`#23262F`).
fn draw_studio_led_indicator(
    painter: &egui::Painter,
    draw_rect: Rect,
    scale_s: f32,
    is_active: bool,
    is_muted: bool,
    theme_mode: ThemeMode,
) {
    let led_w = (draw_rect.width() * 0.25).clamp(15.0, 38.0);
    let led_h = (5.0 * scale_s).clamp(3.8, 7.2);
    let top_offset = (8.2 * scale_s).clamp(5.5, 12.5);
    let led_rect = Rect::from_center_size(
        Pos2::new(draw_rect.center().x, draw_rect.top() + top_offset),
        Vec2::new(led_w, led_h),
    );
    let led_rounding = Rounding::same((1.5 * scale_s).clamp(1.2, 2.4));

    if is_muted {
        // RED STUDIO LED (Muted)
        painter.rect_filled(
            led_rect.expand((2.4 * scale_s).clamp(1.5, 4.5)),
            Rounding::same(3.0 * scale_s),
            Color32::from_rgba_unmultiplied(239, 68, 68, 48),
        );
        painter.rect_filled(led_rect, led_rounding, Color32::from_rgb(239, 68, 68));
        painter.rect_stroke(
            led_rect,
            led_rounding,
            Stroke::new(1.0, Color32::from_rgb(248, 113, 113)),
        );
    } else if is_active {
        // GREEN STUDIO LED (In Use / Active)
        painter.rect_filled(
            led_rect.expand((2.4 * scale_s).clamp(1.5, 4.5)),
            Rounding::same(3.0 * scale_s),
            Color32::from_rgba_unmultiplied(34, 197, 94, 48),
        );
        painter.rect_filled(led_rect, led_rounding, Color32::from_rgb(34, 197, 94));
        painter.rect_stroke(
            led_rect,
            led_rounding,
            Stroke::new(1.0, Color32::from_rgb(74, 222, 128)),
        );
    } else {
        // EMPTY MATTE GREY STUDIO LED SLOT (Unused)
        let (unlit_fill, bezel_stroke) = match theme_mode {
            ThemeMode::Dark => (
                Color32::from_rgb(30, 33, 41),
                Color32::from_rgb(48, 52, 64),
            ),
            ThemeMode::Light => (
                Color32::from_rgb(214, 218, 228),
                Color32::from_rgb(182, 188, 202),
            ),
        };
        painter.rect_filled(led_rect, led_rounding, unlit_fill);
        painter.rect_stroke(led_rect, led_rounding, Stroke::new(1.0, bezel_stroke));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proportional_button_scaling_math() {
        let full = ProportionalButtonMetrics::compute(120.0, 120.0, true);
        let two_thirds = ProportionalButtonMetrics::compute(80.0, 80.0, true);

        let ratio = two_thirds.unit_l / full.unit_l;
        assert!((ratio - (2.0 / 3.0)).abs() < 1e-4);
        assert!((two_thirds.icon_size / full.icon_size - (2.0 / 3.0)).abs() < 1e-4);
        assert!((two_thirds.badge_radius / full.badge_radius - (2.0 / 3.0)).abs() < 1e-4);
        assert!((two_thirds.corner_radius / full.corner_radius - (2.0 / 3.0)).abs() < 1e-4);
    }
}
