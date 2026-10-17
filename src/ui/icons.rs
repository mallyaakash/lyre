use std::collections::HashMap;
use std::fs;
use std::path::Path;

use egui::{Color32, ColorImage, Context, TextureHandle, TextureOptions};
use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg::{Options, Tree};

use crate::config::{IconSource, PresetIcon};

/// Returns clean, scalable 24x24 vector SVG markup for each built-in audio preset.
/// Uses `currentColor` placeholder (`#FFFFFF`) so we can dynamically tint or render it cleanly.
pub fn preset_svg_markup(preset: PresetIcon) -> &'static str {
    match preset {
        PresetIcon::Speakers => {
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#FFFFFF" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
                <polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5" fill="#FFFFFF" fill-opacity="0.18"/>
                <path d="M15.54 8.46a5 5 0 0 1 0 7.07"/>
                <path d="M19.07 4.93a10 10 0 0 1 0 14.14"/>
            </svg>"##
        }
        PresetIcon::Headphones => {
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#FFFFFF" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
                <path d="M3 14v-3a9 9 0 0 1 18 0v3"/>
                <rect x="2" y="13" width="4" height="7" rx="2" fill="#FFFFFF" fill-opacity="0.22"/>
                <rect x="18" y="13" width="4" height="7" rx="2" fill="#FFFFFF" fill-opacity="0.22"/>
            </svg>"##
        }
        PresetIcon::IEMs => {
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#FFFFFF" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
                <circle cx="7.5" cy="9.5" r="3.5" fill="#FFFFFF" fill-opacity="0.22"/>
                <path d="M10.5 11.2l2.2 1.3"/>
                <path d="M6 12.8v5.2a2.5 2.5 0 0 0 5 0"/>
                <circle cx="16.5" cy="9.5" r="3.5" fill="#FFFFFF" fill-opacity="0.22"/>
                <path d="M13.5 11.2l-2.2 1.3"/>
                <path d="M18 12.8v5.2a2.5 2.5 0 0 1-5 0"/>
            </svg>"##
        }
        PresetIcon::StudioMonitors => {
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#FFFFFF" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
                <rect x="4" y="2" width="16" height="20" rx="2.5" fill="#FFFFFF" fill-opacity="0.12"/>
                <circle cx="12" cy="7" r="1.6" fill="#FFFFFF"/>
                <circle cx="12" cy="14.5" r="4.2" fill="#FFFFFF" fill-opacity="0.22"/>
                <circle cx="12" cy="14.5" r="1.2" fill="#FFFFFF"/>
            </svg>"##
        }
        PresetIcon::Microphone => {
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#FFFFFF" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
                <rect x="9" y="2" width="6" height="11" rx="3" fill="#FFFFFF" fill-opacity="0.22"/>
                <path d="M19 10v2a7 7 0 0 1-14 0v-2"/>
                <line x1="12" y1="19" x2="12" y2="22"/>
                <line x1="8" y1="22" x2="16" y2="22"/>
            </svg>"##
        }
        PresetIcon::HeadsetMic => {
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#FFFFFF" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
                <path d="M4 13v-2a8 8 0 0 1 16 0v2"/>
                <rect x="2" y="12" width="3.5" height="6" rx="1.5" fill="#FFFFFF" fill-opacity="0.22"/>
                <rect x="18.5" y="12" width="3.5" height="6" rx="1.5" fill="#FFFFFF" fill-opacity="0.22"/>
                <path d="M5.5 17.5v1a3 3 0 0 0 3 3h3.5"/>
                <circle cx="13.5" cy="21.5" r="1.3" fill="#FFFFFF"/>
            </svg>"##
        }
        PresetIcon::DacAmp => {
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#FFFFFF" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
                <rect x="2" y="6" width="20" height="12" rx="2.5" fill="#FFFFFF" fill-opacity="0.14"/>
                <circle cx="16.5" cy="12" r="3.2" fill="#FFFFFF" fill-opacity="0.25"/>
                <line x1="16.5" y1="12" x2="18.5" y2="10"/>
                <circle cx="6.5" cy="12" r="1.2" fill="#FFFFFF"/>
                <line x1="10" y1="10" x2="11.5" y2="10"/>
                <line x1="10" y1="14" x2="11.5" y2="14"/>
            </svg>"##
        }
    }
}

#[derive(Hash, PartialEq, Eq, Clone)]
struct CacheKey {
    source: IconSource,
    size_bucket: u32,
}

/// Manages rasterized GPU textures for built-in vector presets and user-selected custom media.
#[derive(Default)]
pub struct IconCache {
    textures: HashMap<CacheKey, Option<TextureHandle>>,
}

impl IconCache {
    pub fn new() -> Self {
        Self {
            textures: HashMap::new(),
        }
    }

    /// Invalidates cached textures (e.g. when a custom file is reloaded).
    pub fn clear(&mut self) {
        self.textures.clear();
    }

    /// Returns a crisp `TextureHandle` for the requested `IconSource` at the given target pixel size.
    /// Built-in SVGs are rendered white (`#FFFFFF`) so `egui` can multiply-tint them to any color in 0ms.
    pub fn get_texture(
        &mut self,
        ctx: &Context,
        source: &IconSource,
        desired_px: f32,
    ) -> Option<TextureHandle> {
        let bucket = quantize_size(desired_px);
        let key = CacheKey {
            source: source.clone(),
            size_bucket: bucket,
        };

        if let Some(cached) = self.textures.get(&key) {
            return cached.clone();
        }

        let loaded = match source {
            IconSource::Preset(preset) => {
                let svg_str = preset_svg_markup(*preset);
                rasterize_svg_bytes(svg_str.as_bytes(), bucket)
            }
            IconSource::CustomPath(path) => load_custom_media(path, bucket),
        };

        let handle = loaded.map(|color_img| {
            ctx.load_texture(
                format!("lyre_icon_{:?}_{}", source, bucket),
                color_img,
                TextureOptions::LINEAR,
            )
        });

        self.textures.insert(key, handle.clone());
        handle
    }
}

fn quantize_size(px: f32) -> u32 {
    let raw = px.ceil() as u32;
    match raw {
        0..=32 => 32,
        33..=48 => 48,
        49..=64 => 64,
        65..=96 => 96,
        97..=128 => 128,
        129..=192 => 192,
        _ => 256,
    }
}

pub fn rasterize_svg_bytes(svg_bytes: &[u8], target_size: u32) -> Option<ColorImage> {
    let opt = Options::default();
    let tree = Tree::from_data(svg_bytes, &opt).ok()?;
    let size = tree.size();
    if size.width() <= 0.0 || size.height() <= 0.0 {
        return None;
    }

    let mut pixmap = Pixmap::new(target_size, target_size)?;
    let scale_x = target_size as f32 / size.width();
    let scale_y = target_size as f32 / size.height();
    let scale = scale_x.min(scale_y);

    let offset_x = (target_size as f32 - size.width() * scale) * 0.5;
    let offset_y = (target_size as f32 - size.height() * scale) * 0.5;

    let transform = Transform::from_scale(scale, scale).post_translate(offset_x, offset_y);
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    // Convert premultiplied RGBA from tiny-skia to unpremultiplied RGBA for egui::ColorImage
    let mut pixels = Vec::with_capacity((target_size * target_size) as usize);
    for p in pixmap.pixels() {
        let a = p.alpha();
        if a == 0 {
            pixels.push(Color32::TRANSPARENT);
        } else {
            let r = ((p.red() as u32 * 255) / a as u32).min(255) as u8;
            let g = ((p.green() as u32 * 255) / a as u32).min(255) as u8;
            let b = ((p.blue() as u32 * 255) / a as u32).min(255) as u8;
            pixels.push(Color32::from_rgba_unmultiplied(r, g, b, a));
        }
    }

    Some(ColorImage {
        size: [target_size as usize, target_size as usize],
        pixels,
    })
}

fn load_custom_media(path: &Path, target_size: u32) -> Option<ColorImage> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let bytes = fs::read(path).ok()?;
    load_custom_media_bytes(&bytes, &ext, target_size)
}

pub fn load_custom_media_bytes(bytes: &[u8], ext: &str, target_size: u32) -> Option<ColorImage> {
    if ext.eq_ignore_ascii_case("svg") {
        return rasterize_svg_bytes(bytes, target_size);
    }

    let dyn_img = image::load_from_memory(bytes).ok()?;
    let resized = dyn_img.resize(
        target_size,
        target_size,
        image::imageops::FilterType::Lanczos3,
    );
    let rgba = resized.to_rgba8();
    let (w, h) = rgba.dimensions();

    // Center onto a square transparent canvas so aspect ratio and alpha transparency are preserved
    let mut canvas = vec![Color32::TRANSPARENT; (target_size * target_size) as usize];
    let offset_x = target_size.saturating_sub(w) / 2;
    let offset_y = target_size.saturating_sub(h) / 2;

    for y in 0..h.min(target_size) {
        for x in 0..w.min(target_size) {
            let p = rgba.get_pixel(x, y).0;
            let dst_idx = ((offset_y + y) * target_size + (offset_x + x)) as usize;
            if dst_idx < canvas.len() {
                canvas[dst_idx] = if p[3] == 0 {
                    Color32::TRANSPARENT
                } else {
                    Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3])
                };
            }
        }
    }

    Some(ColorImage {
        size: [target_size as usize, target_size as usize],
        pixels: canvas,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_seven_presets_rasterize_cleanly() {
        for preset in PresetIcon::ALL {
            let svg = preset_svg_markup(preset);
            let img = rasterize_svg_bytes(svg.as_bytes(), 64)
                .unwrap_or_else(|| panic!("Failed to rasterize preset {:?}", preset));
            assert_eq!(img.size, [64, 64]);
            assert!(
                img.pixels.iter().any(|p| p.a() > 0),
                "Preset {:?} produced an empty transparent image",
                preset
            );
        }
    }

    #[test]
    fn test_webp_and_png_transparency_loading() {
        let mut raw_img = image::RgbaImage::new(32, 16);
        for y in 0..16 {
            for x in 0..32 {
                if x < 16 {
                    // Transparent left half
                    raw_img.put_pixel(x, y, image::Rgba([0, 0, 0, 0]));
                } else {
                    // Semi-transparent cyan right half
                    raw_img.put_pixel(x, y, image::Rgba([6, 182, 212, 180]));
                }
            }
        }

        let mut webp_bytes = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(raw_img)
            .write_to(&mut webp_bytes, image::ImageFormat::WebP)
            .expect("Should encode lossless WebP with alpha");

        let loaded = load_custom_media_bytes(webp_bytes.get_ref(), "webp", 64)
            .expect("Should decode WebP with alpha");
        assert_eq!(loaded.size, [64, 64]);
        // Top-left padding and left half must be transparent
        assert_eq!(loaded.pixels[0], Color32::TRANSPARENT);
        // Right half must contain non-zero alpha pixels
        assert!(loaded.pixels.iter().any(|p| p.a() > 0 && p.b() > 100));
    }
}
