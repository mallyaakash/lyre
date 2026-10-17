use egui::{
    Align2, Color32, CursorIcon, FontFamily, FontId, Pos2, Rect, Rounding, Sense, Stroke, Ui, Vec2,
};

use crate::audio::EndpointDirection;
use crate::config::{ButtonConfig, IconSource, ThemeMode};
use crate::ui::icons::IconCache;
use crate::ui::theme::resolve_button_palette;

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
        let corner_radius = (0.14 * unit_l).clamp(4.0, 24.0);
        let base_stroke_width = (1.35 * scale_s).clamp(1.0, 4.0);

        let (icon_size, icon_center_offset_y) = if show_label {
            (0.41 * unit_l, -0.06 * unit_l)
        } else {
            (0.52 * unit_l, 0.04 * unit_l)
        };

        let badge_radius = (0.20 * icon_size).max(5.5);
        let badge_offset = Vec2::new(0.38 * icon_size, -0.36 * icon_size);
        let badge_font_size = (1.26 * badge_radius).clamp(7.0, 34.0);

        let label_font_size = (0.125 * unit_l).clamp(7.5, 26.0);
        let label_offset_y = 0.30 * unit_l;

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
/// with a frosted glass aesthetic and a studio-equipment rectangular LED indicator.
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
        cell_rect.shrink(2.0)
    } else if is_pressed && !menu_hovered {
        cell_rect.shrink(1.0)
    } else {
        cell_rect
    };

    let painter = ui.painter_at(cell_rect.expand(8.0 * metrics.scale_s));
    let rounding = Rounding::same(metrics.corner_radius);

    // 1. Soft drop shadow + outer ambient glow for depth
    let shadow_offset = Vec2::new(0.0, (2.0 * metrics.scale_s).clamp(1.0, 4.0));
    painter.rect_filled(
        draw_rect.translate(shadow_offset),
        rounding,
        match theme_mode {
            ThemeMode::Dark => Color32::from_rgba_unmultiplied(0, 0, 0, 110),
            ThemeMode::Light => Color32::from_rgba_unmultiplied(0, 0, 0, 32),
        },
    );

    if let Some(glow) = palette.glow_color {
        let glow_expand = (2.6 * metrics.scale_s).clamp(1.5, 6.0);
        painter.rect_filled(
            draw_rect.expand(glow_expand),
            Rounding::same(metrics.corner_radius + glow_expand * 0.55),
            glow,
        );
    }

    // 2. Frosted Glass Card Base Fill
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

    // 3. Multi-layered Glassy Reflection & Specular Rim
    //    3a. Upper glass reflection band (top 46% of the button)
    let upper_glass_rect = Rect::from_min_max(
        draw_rect.min + Vec2::new(1.2, 1.2),
        Pos2::new(
            draw_rect.max.x - 1.2,
            draw_rect.min.y + draw_rect.height() * 0.46,
        ),
    );
    let upper_glass_fill = match theme_mode {
        ThemeMode::Dark => {
            if is_hovered || is_active {
                Color32::from_rgba_unmultiplied(255, 255, 255, 19)
            } else {
                Color32::from_rgba_unmultiplied(255, 255, 255, 12)
            }
        }
        ThemeMode::Light => Color32::from_rgba_unmultiplied(255, 255, 255, 95),
    };
    painter.rect_filled(
        upper_glass_rect,
        Rounding {
            nw: (metrics.corner_radius - 1.2).max(2.0),
            ne: (metrics.corner_radius - 1.2).max(2.0),
            sw: (metrics.corner_radius * 0.35).max(2.0),
            se: (metrics.corner_radius * 0.35).max(2.0),
        },
        upper_glass_fill,
    );

    //    3b. Inner glass bevel ring (1px bright inner border for frosted acrylic look)
    let inner_glass_stroke = match theme_mode {
        ThemeMode::Dark => Color32::from_rgba_unmultiplied(255, 255, 255, if is_hovered { 34 } else { 20 }),
        ThemeMode::Light => Color32::from_rgba_unmultiplied(255, 255, 255, 150),
    };
    painter.rect_stroke(
        draw_rect.shrink(1.2),
        Rounding::same((metrics.corner_radius - 1.2).max(2.0)),
        Stroke::new(1.0, inner_glass_stroke),
    );

    //    3c. Crisp top specular highlight line along the top glass edge
    let specular_inset = (metrics.corner_radius * 0.75).max(4.0);
    if draw_rect.width() > specular_inset * 2.2 {
        let spec_y = draw_rect.top() + 1.5;
        let spec_color = match theme_mode {
            ThemeMode::Dark => Color32::from_rgba_unmultiplied(255, 255, 255, 58),
            ThemeMode::Light => Color32::from_rgba_unmultiplied(255, 255, 255, 210),
        };
        painter.line_segment(
            [
                Pos2::new(draw_rect.left() + specular_inset, spec_y),
                Pos2::new(draw_rect.right() - specular_inset, spec_y),
            ],
            Stroke::new(1.2, spec_color),
        );
    }

    // 4. Outer Glass Border Stroke
    let stroke_width = if is_being_dragged {
        (metrics.base_stroke_width * 2.2).max(2.0)
    } else {
        metrics.base_stroke_width * palette.border_width_mult
    };
    painter.rect_stroke(
        draw_rect,
        rounding,
        Stroke::new(stroke_width, palette.border_color),
    );

    // 5. Studio Equipment Rectangular LED Indicator Light (Top-Center)
    //    - Muted  -> Glowing Red rectangular studio light
    //    - Active -> Glowing Green rectangular studio light
    //    - Unused -> Empty recessed Grey rectangular studio light
    draw_studio_led_indicator(
        &painter,
        draw_rect,
        metrics.scale_s,
        is_active,
        is_muted,
        theme_mode,
    );

    // 6. Centered Device Icon (Built-in SVG or Custom Uploaded Media with full RGBA transparency)
    let icon_center = Pos2::new(
        draw_rect.center().x,
        draw_rect.center().y + metrics.icon_center_offset_y,
    );
    let icon_rect = Rect::from_center_size(icon_center, Vec2::splat(metrics.icon_size));

    if let Some(tex) = icon_cache.get_texture(ui.ctx(), &btn.icon, metrics.icon_size * 1.5) {
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

    // 7. Optional Circular Corner Number Badge (1, 2, 3...) overlapping top-right of the icon
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
            FontId::new(metrics.badge_font_size, FontFamily::Proportional),
            palette.badge_fg,
        );
    }

    // 8. Subtle Input ("MIC") / Output ("OUT") micro-tag in top-left corner
    if btn.direction == EndpointDirection::Input || edit_mode {
        let tag_font = (0.085 * metrics.unit_l).clamp(6.5, 11.0);
        let tag_text = match btn.direction {
            EndpointDirection::Output => "OUT",
            EndpointDirection::Input => "MIC",
        };
        let tag_pos = Pos2::new(
            draw_rect.left() + (7.5 * metrics.scale_s).max(5.5),
            draw_rect.top() + (6.5 * metrics.scale_s).max(5.0),
        );
        painter.text(
            tag_pos,
            Align2::LEFT_TOP,
            tag_text,
            FontId::new(tag_font, FontFamily::Proportional),
            palette.subtext_color,
        );
    }

    // 9. Optional Text Label ("Icon + Text Title" vs "Icon-Only")
    if btn.show_label && !btn.title.is_empty() {
        let label_pos = Pos2::new(
            draw_rect.center().x,
            draw_rect.center().y + metrics.label_offset_y,
        );
        let max_chars = ((draw_rect.width() * 0.88) / (metrics.label_font_size * 0.53))
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
            FontId::new(metrics.label_font_size, FontFamily::Proportional),
            palette.text_color,
        );
    }

    // 10. Three Vertical Ellipses (`⋮`) Edit Button in the Top-Right Corner
    //     Always visible in Edit Mode, and visible on hover in Minimalist Mode.
    if edit_mode || is_hovered {
        if menu_hovered {
            let hover_pill = match theme_mode {
                ThemeMode::Dark => Color32::from_rgba_unmultiplied(255, 255, 255, 42),
                ThemeMode::Light => Color32::from_rgba_unmultiplied(0, 0, 0, 28),
            };
            painter.rect_filled(menu_rect, Rounding::same(menu_rect.width() * 0.35), hover_pill);
        }

        let dot_r = (1.5 * metrics.scale_s).clamp(1.2, 2.5);
        let dot_spacing = (4.4 * metrics.scale_s).clamp(3.3, 7.0);
        let dot_alpha = if menu_hovered {
            255
        } else if is_hovered {
            210
        } else {
            145
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
}

/// Draws a hardware studio-equipment rectangular LED light indicator near the top-center of a button:
/// - **Muted (`is_muted`):** Lit Red rectangular studio LED with outer crimson bloom.
/// - **Active (`is_active && !is_muted`):** Lit Green rectangular studio LED with outer emerald bloom.
/// - **Unused (`!is_active && !is_muted`):** Empty recessed Grey rectangular LED housing.
fn draw_studio_led_indicator(
    painter: &egui::Painter,
    draw_rect: Rect,
    scale_s: f32,
    is_active: bool,
    is_muted: bool,
    theme_mode: ThemeMode,
) {
    let led_w = (draw_rect.width() * 0.28).clamp(16.0, 44.0);
    let led_h = (5.8 * scale_s).clamp(4.2, 8.5);
    let top_offset = (8.5 * scale_s).clamp(5.5, 13.0);
    let led_rect = Rect::from_center_size(
        Pos2::new(draw_rect.center().x, draw_rect.top() + top_offset),
        Vec2::new(led_w, led_h),
    );
    let led_rounding = Rounding::same((1.6 * scale_s).clamp(1.2, 2.8));

    if is_muted {
        // RED STUDIO LED (Muted)
        painter.rect_filled(
            led_rect.expand((3.2 * scale_s).clamp(2.0, 5.5)),
            Rounding::same(3.5 * scale_s),
            Color32::from_rgba_unmultiplied(239, 68, 68, 52),
        );
        painter.rect_filled(
            led_rect.expand((1.4 * scale_s).clamp(1.0, 2.5)),
            Rounding::same(2.4 * scale_s),
            Color32::from_rgba_unmultiplied(239, 68, 68, 105),
        );
        // LED body
        painter.rect_filled(led_rect, led_rounding, Color32::from_rgb(239, 68, 68));
        // Inner bright filament hotspot
        let core_rect = led_rect.shrink2(Vec2::new(led_w * 0.14, led_h * 0.26));
        painter.rect_filled(
            core_rect,
            Rounding::same(1.0),
            Color32::from_rgb(254, 202, 202),
        );
        // Crisp studio bezel frame
        painter.rect_stroke(
            led_rect,
            led_rounding,
            Stroke::new(1.0, Color32::from_rgb(153, 27, 27)),
        );
    } else if is_active {
        // GREEN STUDIO LED (In Use / Active)
        painter.rect_filled(
            led_rect.expand((3.4 * scale_s).clamp(2.0, 6.0)),
            Rounding::same(3.5 * scale_s),
            Color32::from_rgba_unmultiplied(34, 197, 94, 55),
        );
        painter.rect_filled(
            led_rect.expand((1.5 * scale_s).clamp(1.0, 2.5)),
            Rounding::same(2.4 * scale_s),
            Color32::from_rgba_unmultiplied(34, 197, 94, 115),
        );
        // LED body
        painter.rect_filled(led_rect, led_rounding, Color32::from_rgb(34, 197, 94));
        // Inner bright filament hotspot
        let core_rect = led_rect.shrink2(Vec2::new(led_w * 0.14, led_h * 0.26));
        painter.rect_filled(
            core_rect,
            Rounding::same(1.0),
            Color32::from_rgb(187, 247, 208),
        );
        // Crisp studio bezel frame
        painter.rect_stroke(
            led_rect,
            led_rounding,
            Stroke::new(1.0, Color32::from_rgb(21, 128, 61)),
        );
    } else {
        // EMPTY GREY STUDIO LED (Unused / Unlit Recessed Lens)
        let (unlit_fill, bezel_stroke) = match theme_mode {
            ThemeMode::Dark => (
                Color32::from_rgb(34, 35, 40),
                Color32::from_rgb(66, 68, 78),
            ),
            ThemeMode::Light => (
                Color32::from_rgb(206, 209, 218),
                Color32::from_rgb(156, 163, 175),
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
