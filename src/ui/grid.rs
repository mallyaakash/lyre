use std::collections::HashMap;
use std::time::{Duration, Instant};

use egui::{Id, PointerButton, Pos2, Rect, Ui, Vec2};

use crate::audio::AudioEngine;
use crate::config::{AppConfig, ButtonConfig};
use crate::ui::button_widget::{
    compute_ellipsis_menu_rect, draw_audio_button, draw_drop_target_slot, ButtonVolumePopup,
    ProportionalButtonMetrics,
};
use crate::ui::icons::IconCache;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GridEvent {
    None,
    SwitchDevice(usize),
    ToggleMuteDevice(usize),
    AdjustVolume { button_idx: usize, delta_pct: i32 },
    EditButton(usize),
    MoveButton { from: usize, to: usize },
    OpenContextMenu { button_idx: Option<usize> },
    StartWindowDrag,
    AddFirstButton,
}

#[derive(Debug, Clone, Default)]
struct GridPointerTracker {
    primary_pressed_btn_id: Option<String>,
    primary_press_pos: Option<Pos2>,
    drag_grab_offset: Vec2,
    is_reordering: bool,
    middle_pressed_btn_idx: Option<usize>,
    secondary_press_pos: Option<Pos2>,
    secondary_dragged: bool,
    scroll_accum_y: f32,
}

#[derive(Debug, Clone, Default)]
struct GridAnimState {
    vu_levels: HashMap<String, f32>,
    last_vu_tick: Option<Instant>,
    volume_popups: HashMap<String, (u32, bool, Instant)>,
}

/// Triggers the brief scroll-wheel volume / middle-click mute HUD popup on a specific button card.
pub fn trigger_button_volume_popup(
    ctx: &egui::Context,
    button_id: &str,
    volume_pct: u32,
    is_muted: bool,
) {
    let anim_id = Id::new("lyre_grid_anim_state");
    ctx.data_mut(|d| {
        let mut state = d.get_temp::<GridAnimState>(anim_id).unwrap_or_default();
        state.volume_popups.insert(
            button_id.to_string(),
            (volume_pct.min(100), is_muted, Instant::now()),
        );
        d.insert_temp(anim_id, state);
    });
    ctx.request_repaint();
}

/// Computes the exact cell `Rect`s for $B$ buttons arranged in $N$ columns inside `bounds`.
pub fn calculate_grid_cells(bounds: Rect, columns: usize, button_count: usize) -> Vec<Rect> {
    if button_count == 0 {
        return Vec::new();
    }
    let cols = columns.max(1);
    let rows = ((button_count + cols - 1) / cols).max(1);

    let w = bounds.width().max(10.0);
    let h = bounds.height().max(10.0);

    let raw_cell_w = w / (cols as f32);
    let raw_cell_h = h / (rows as f32);
    let pad = (raw_cell_w.min(raw_cell_h) * 0.055).clamp(3.0, 12.0);

    let total_pad_x = pad * ((cols + 1) as f32);
    let total_pad_y = pad * ((rows + 1) as f32);

    let cell_w = ((w - total_pad_x) / (cols as f32)).max(12.0);
    let cell_h = ((h - total_pad_y) / (rows as f32)).max(12.0);

    let mut rects = Vec::with_capacity(button_count);
    for idx in 0..button_count {
        let col = idx % cols;
        let row = idx / cols;

        let x = bounds.left() + pad + (col as f32) * (cell_w + pad);
        let y = bounds.top() + pad + (row as f32) * (cell_h + pad);
        rects.push(Rect::from_min_size(
            Pos2::new(x, y),
            Vec2::new(cell_w, cell_h),
        ));
    }
    rects
}

/// Renders the responsive button grid filling `available_rect` with deterministic
/// click-to-switch, drag-to-reorder, and right-click context menu handling.
pub fn render_button_grid(
    ui: &mut Ui,
    available_rect: Rect,
    config: &AppConfig,
    audio: &AudioEngine,
    edit_mode: bool,
    icon_cache: &mut IconCache,
) -> GridEvent {
    let tracker_id = Id::new("lyre_grid_pointer_tracker");
    let mut tracker: GridPointerTracker = ui
        .ctx()
        .data_mut(|d| d.get_temp::<GridPointerTracker>(tracker_id))
        .unwrap_or_default();

    let (
        primary_pressed,
        primary_down,
        primary_released,
        middle_pressed,
        middle_released,
        secondary_pressed,
        secondary_down,
        secondary_released,
        interact_pos,
        hover_pos,
        alt_held,
        wheel_line_notches,
        raw_scroll_y,
    ) = ui.input(|i| {
        let mut line_notches: i32 = 0;
        for ev in &i.events {
            if let egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Line,
                delta,
                ..
            } = ev
            {
                if delta.y.abs() > 0.01 {
                    line_notches += if delta.y > 0.0 {
                        (delta.y.ceil() as i32).max(1)
                    } else {
                        (delta.y.floor() as i32).min(-1)
                    };
                }
            }
        }
        (
            i.pointer.button_pressed(PointerButton::Primary),
            i.pointer.primary_down(),
            i.pointer.button_released(PointerButton::Primary),
            i.pointer.button_pressed(PointerButton::Middle),
            i.pointer.button_released(PointerButton::Middle),
            i.pointer.button_pressed(PointerButton::Secondary),
            i.pointer.secondary_down(),
            i.pointer.button_released(PointerButton::Secondary),
            i.pointer.interact_pos().or_else(|| i.pointer.latest_pos()),
            i.pointer.hover_pos().or_else(|| i.pointer.latest_pos()),
            i.modifiers.alt,
            line_notches,
            i.raw_scroll_delta.y,
        )
    });

    if config.buttons.is_empty() {
        let mut action = GridEvent::None;
        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(available_rect), |ui| {
            ui.centered_and_justified(|ui| {
                if ui.button("+ Click to Add Audio Device Button").clicked() {
                    action = GridEvent::AddFirstButton;
                }
            });
        });
        if secondary_released {
            if let Some(pos) = interact_pos {
                if available_rect.contains(pos) {
                    return GridEvent::OpenContextMenu { button_idx: None };
                }
            }
        }
        return action;
    }

    let cells = calculate_grid_cells(
        available_rect,
        config.window.columns,
        config.buttons.len(),
    );

    let mut triggered_event = GridEvent::None;

    // 1. Handle Primary Press / Drag-to-Reorder / Release deterministically
    if primary_pressed {
        if let Some(pos) = interact_pos {
            if available_rect.contains(pos) {
                if alt_held {
                    triggered_event = GridEvent::StartWindowDrag;
                } else if let Some(hit_idx) = cells.iter().position(|r| r.contains(pos)) {
                    tracker.primary_pressed_btn_id = Some(config.buttons[hit_idx].id.clone());
                    tracker.primary_press_pos = Some(pos);
                    tracker.drag_grab_offset = pos - cells[hit_idx].center();
                    tracker.is_reordering = false;
                } else {
                    tracker.primary_pressed_btn_id = None;
                    tracker.primary_press_pos = None;
                    tracker.is_reordering = false;
                }
            }
        }
    }

    if primary_down && !alt_held {
        if let (Some(ref drag_id), Some(origin), Some(curr_pos)) = (
            &tracker.primary_pressed_btn_id,
            tracker.primary_press_pos,
            interact_pos,
        ) {
            // Enter drag-to-reorder mode once pointer moves more than 12 pixels from press origin
            if (curr_pos - origin).length() > 12.0 {
                tracker.is_reordering = true;
            }
            if tracker.is_reordering {
                if let Some(from_idx) = config.buttons.iter().position(|b| &b.id == drag_id) {
                    if let Some(to_idx) = cells.iter().position(|r| r.contains(curr_pos)) {
                        if from_idx != to_idx {
                            triggered_event = GridEvent::MoveButton {
                                from: from_idx,
                                to: to_idx,
                            };
                        }
                    }
                }
            }
        }
    }

    if primary_released {
        if let (Some(ref pressed_id), Some(rel_pos)) = (&tracker.primary_pressed_btn_id, interact_pos) {
            if !tracker.is_reordering && !alt_held {
                if let Some(btn_idx) = config.buttons.iter().position(|b| &b.id == pressed_id) {
                    let cell_rect = cells[btn_idx];
                    // Allow click if released inside the button cell (or within 14px of press origin)
                    let close_to_origin = tracker
                        .primary_press_pos
                        .map(|orig| (rel_pos - orig).length() <= 14.0)
                        .unwrap_or(false);

                    if cell_rect.contains(rel_pos) || close_to_origin {
                        let metrics = ProportionalButtonMetrics::compute(
                            cell_rect.width(),
                            cell_rect.height(),
                            config.buttons[btn_idx].show_label,
                        );
                        let menu_rect = compute_ellipsis_menu_rect(cell_rect, metrics.scale_s);
                        if menu_rect.expand(3.0).contains(rel_pos) {
                            triggered_event = GridEvent::EditButton(btn_idx);
                        } else {
                            triggered_event = GridEvent::SwitchDevice(btn_idx);
                        }
                    }
                }
            }
        }
        tracker.primary_pressed_btn_id = None;
        tracker.primary_press_pos = None;
        tracker.is_reordering = false;
    }

    if !primary_down && !primary_released {
        tracker.primary_pressed_btn_id = None;
        tracker.primary_press_pos = None;
        tracker.is_reordering = false;
    }

    // 2. Handle Middle-Click (Scroll-Wheel Click) to Mute/Unmute a Single Device
    if middle_pressed {
        if let Some(pos) = interact_pos.or(hover_pos) {
            if available_rect.contains(pos) {
                tracker.middle_pressed_btn_idx = cells.iter().position(|r| r.contains(pos));
            }
        }
    }
    if middle_released {
        if let (Some(pressed_idx), Some(pos)) =
            (tracker.middle_pressed_btn_idx.take(), interact_pos.or(hover_pos))
        {
            if pressed_idx < cells.len() && cells[pressed_idx].expand(6.0).contains(pos) {
                triggered_event = GridEvent::ToggleMuteDevice(pressed_idx);
            }
        }
    }

    // 3. Handle Hover + Scroll-Wheel Volume Adjustment (2% increments per wheel notch)
    if let Some(pos) = hover_pos.or(interact_pos) {
        if available_rect.contains(pos) {
            if let Some(hover_idx) = cells.iter().position(|r| r.contains(pos)) {
                let mut notches = wheel_line_notches;
                if notches == 0 && raw_scroll_y.abs() > 0.5 {
                    tracker.scroll_accum_y += raw_scroll_y;
                    if tracker.scroll_accum_y.abs() >= 18.0 {
                        let step = (tracker.scroll_accum_y / 24.0).trunc() as i32;
                        notches = if step != 0 {
                            step
                        } else if tracker.scroll_accum_y > 0.0 {
                            1
                        } else {
                            -1
                        };
                        tracker.scroll_accum_y = 0.0;
                    }
                } else if notches != 0 {
                    tracker.scroll_accum_y = 0.0;
                }

                if notches != 0 {
                    // Each scroll notch adjusts device volume by 2%
                    triggered_event = GridEvent::AdjustVolume {
                        button_idx: hover_idx,
                        delta_pct: notches * 2,
                    };
                }
            } else {
                tracker.scroll_accum_y = 0.0;
            }
        }
    }

    // 4. Handle Secondary (Right-Click) Press / Drag (Window Move) / Release (Context Menu)
    if secondary_pressed {
        if let Some(pos) = interact_pos {
            if available_rect.contains(pos) {
                tracker.secondary_press_pos = Some(pos);
                tracker.secondary_dragged = false;
            }
        }
    }

    if secondary_down {
        if let (Some(origin), Some(curr)) = (tracker.secondary_press_pos, interact_pos) {
            if (curr - origin).length() > 10.0 {
                tracker.secondary_dragged = true;
                triggered_event = GridEvent::StartWindowDrag;
            }
        }
    }

    if secondary_released {
        if !tracker.secondary_dragged {
            if let Some(pos) = interact_pos.or(tracker.secondary_press_pos) {
                if available_rect.contains(pos) {
                    let hit_btn = cells.iter().position(|r| r.contains(pos));
                    triggered_event = GridEvent::OpenContextMenu {
                        button_idx: hit_btn,
                    };
                }
            }
        }
        tracker.secondary_press_pos = None;
        tracker.secondary_dragged = false;
    }

    ui.ctx().data_mut(|d| d.insert_temp(tracker_id, tracker.clone()));

    // 5. Update Silky-Smooth Studio VU Meter & Brief Volume Popup HUD Animation State
    let anim_id = Id::new("lyre_grid_anim_state");
    let mut anim_state: GridAnimState = ui
        .ctx()
        .data_mut(|d| d.get_temp::<GridAnimState>(anim_id))
        .unwrap_or_default();

    let now = Instant::now();
    let dt = anim_state
        .last_vu_tick
        .map(|prev| now.saturating_duration_since(prev).as_secs_f32().clamp(0.001, 0.12))
        .unwrap_or(0.016);
    anim_state.last_vu_tick = Some(now);

    let mut needs_fast_repaint = false;
    if config.window.show_vu_meter {
        for btn in &config.buttons {
            let raw_peak = audio.get_device_peak_level(&btn.device_id, btn.direction);
            // Perceptual studio curve so moderate audio levels produce a lush, smooth background meter
            let target = if raw_peak > 0.002 {
                raw_peak.powf(0.62).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let prev = anim_state.vu_levels.get(&btn.id).copied().unwrap_or(0.0);
            let tau = if target > prev { 0.032 } else { 0.18 };
            let alpha = 1.0 - (-dt / tau).exp();
            let mut next = prev + (target - prev) * alpha;
            if next < 0.003 && target == 0.0 {
                next = 0.0;
            }
            anim_state.vu_levels.insert(btn.id.clone(), next);
            if next > 0.004 || raw_peak > 0.002 {
                needs_fast_repaint = true;
            }
        }
    } else {
        anim_state.vu_levels.clear();
    }

    // Prune expired volume popups (> 1.15s) and compute current fade-out opacities
    let popup_total_secs = 1.15_f32;
    let popup_hold_secs = 0.75_f32;
    let mut active_popups: HashMap<String, ButtonVolumePopup> = HashMap::new();
    anim_state.volume_popups.retain(|btn_id, (vol_pct, is_muted, ts)| {
        let elapsed = now.saturating_duration_since(*ts).as_secs_f32();
        if elapsed < popup_total_secs {
            let opacity = if elapsed <= popup_hold_secs {
                1.0
            } else {
                let t = (elapsed - popup_hold_secs) / (popup_total_secs - popup_hold_secs);
                (1.0 - t.clamp(0.0, 1.0)).powi(2)
            };
            active_popups.insert(
                btn_id.clone(),
                ButtonVolumePopup {
                    volume_pct: *vol_pct,
                    is_muted: *is_muted,
                    opacity,
                },
            );
            needs_fast_repaint = true;
            true
        } else {
            false
        }
    });

    ui.ctx()
        .data_mut(|d| d.insert_temp(anim_id, anim_state));

    if needs_fast_repaint {
        ui.ctx().request_repaint_after(Duration::from_millis(16));
    } else if config.window.show_vu_meter && config.window.visible {
        // Lightweight 100ms heartbeat when silent so the VU meter wakes up immediately when audio starts
        ui.ctx().request_repaint_after(Duration::from_millis(100));
    }

    // 6. Draw all non-dragged buttons first, and render a drop-target slot + floating card for any dragged button
    let mut dragged_render_info: Option<(&ButtonConfig, Rect, bool, bool, bool, f32, Option<ButtonVolumePopup>)> = None;

    for (bullet_btn, cell_rect) in config.buttons.iter().zip(cells.iter().copied()) {
        let is_active = audio.is_device_active(&bullet_btn.device_id, bullet_btn.direction);
        let is_muted = audio.is_device_muted(&bullet_btn.device_id, bullet_btn.direction);
        let is_present = audio.find_endpoint(&bullet_btn.device_id).is_some();
        let is_pressed_this = primary_down
            && !tracker.is_reordering
            && tracker.primary_pressed_btn_id.as_deref() == Some(bullet_btn.id.as_str());
        let is_being_dragged = primary_down
            && tracker.is_reordering
            && tracker.primary_pressed_btn_id.as_deref() == Some(bullet_btn.id.as_str());
        let vu_level = if config.window.show_vu_meter {
            ui.ctx()
                .data_mut(|d| d.get_temp::<GridAnimState>(anim_id))
                .and_then(|s| s.vu_levels.get(&bullet_btn.id).copied())
                .unwrap_or(0.0)
        } else {
            0.0
        };
        let popup = active_popups.get(&bullet_btn.id).copied();

        if is_being_dragged {
            // Draw the recessed drop-target slot at the button's grid cell
            draw_drop_target_slot(ui, cell_rect, &bullet_btn.color_hex, config.window.theme_mode);
            dragged_render_info = Some((
                bullet_btn,
                cell_rect,
                is_active,
                is_muted,
                is_present,
                vu_level,
                popup,
            ));
        } else {
            draw_audio_button(
                ui,
                cell_rect,
                bullet_btn,
                is_active,
                is_muted,
                is_present,
                is_pressed_this,
                false,
                vu_level,
                popup,
                config.window.theme_mode,
                edit_mode,
                icon_cache,
            );
        }
    }

    // Draw the dragged button card on top, smoothly following the cursor
    if let Some((drag_btn, cell_rect, is_active, is_muted, is_present, vu_level, popup)) =
        dragged_render_info
    {
        let floating_rect = if let Some(curr_pos) = interact_pos {
            let target_center = curr_pos - tracker.drag_grab_offset;
            let raw_rect = Rect::from_center_size(target_center, cell_rect.size());
            let min_x = raw_rect
                .min
                .x
                .clamp(available_rect.left() + 2.0, (available_rect.right() - cell_rect.width() - 2.0).max(available_rect.left() + 2.0));
            let min_y = raw_rect
                .min
                .y
                .clamp(available_rect.top() + 2.0, (available_rect.bottom() - cell_rect.height() - 2.0).max(available_rect.top() + 2.0));
            Rect::from_min_size(Pos2::new(min_x, min_y), cell_rect.size())
        } else {
            cell_rect
        };

        draw_audio_button(
            ui,
            floating_rect,
            drag_btn,
            is_active,
            is_muted,
            is_present,
            false,
            true,
            vu_level,
            popup,
            config.window.theme_mode,
            edit_mode,
            icon_cache,
        );
    }

    triggered_event
}

/// Helper to create a new default `ButtonConfig` bound to the first available audio device.
pub fn create_new_button_from_audio(audio: &AudioEngine, existing_count: usize) -> ButtonConfig {
    use crate::audio::EndpointDirection;
    use crate::config::{shorten_device_title, IconSource, PresetIcon, DEFAULT_COLOR_PALETTE};

    let snap = audio.snapshot();
    let color = DEFAULT_COLOR_PALETTE[existing_count % DEFAULT_COLOR_PALETTE.len()].to_string();

    if let Some(first_out) = snap
        .outputs
        .iter()
        .find(|ep| !crate::audio::is_sonar_virtual_name(&ep.name))
        .or_else(|| snap.outputs.first())
    {
        let preset = PresetIcon::infer_from_name(&first_out.name, EndpointDirection::Output);
        return ButtonConfig::new(
            first_out.id.clone(),
            EndpointDirection::Output,
            shorten_device_title(&first_out.name),
            color,
            IconSource::Preset(preset),
        );
    }

    if let Some(first_in) = snap.inputs.first() {
        let preset = PresetIcon::infer_from_name(&first_in.name, EndpointDirection::Input);
        return ButtonConfig::new(
            first_in.id.clone(),
            EndpointDirection::Input,
            shorten_device_title(&first_in.name),
            color,
            IconSource::Preset(preset),
        );
    }

    ButtonConfig::new(
        String::new(),
        EndpointDirection::Output,
        "Audio Device".to_string(),
        color,
        IconSource::Preset(PresetIcon::Speakers),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::EndpointDirection;
    use crate::config::{IconSource, PresetIcon};

    #[test]
    fn test_grid_cell_proportional_scaling() {
        let rect_large = Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 300.0));
        let rect_small = Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 200.0));

        let cells_large = calculate_grid_cells(rect_large, 3, 6);
        let cells_small = calculate_grid_cells(rect_small, 3, 6);

        assert_eq!(cells_large.len(), 6);
        assert_eq!(cells_small.len(), 6);

        let w_ratio = cells_small[0].width() / cells_large[0].width();
        let h_ratio = cells_small[0].height() / cells_large[0].height();
        assert!((w_ratio - (2.0 / 3.0)).abs() < 0.02);
        assert!((h_ratio - (2.0 / 3.0)).abs() < 0.02);
    }

    #[test]
    fn test_simulated_click_fires_switch_device() {
        let audio = AudioEngine::new().expect("AudioEngine init");
        let mut cfg = AppConfig::default();
        cfg.window.columns = 2;
        cfg.buttons.push(ButtonConfig::new(
            "dev_0".to_string(),
            EndpointDirection::Output,
            "Device 0".to_string(),
            "#3B82F6".to_string(),
            IconSource::Preset(PresetIcon::Speakers),
        ));
        cfg.buttons.push(ButtonConfig::new(
            "dev_1".to_string(),
            EndpointDirection::Output,
            "Device 1".to_string(),
            "#10B981".to_string(),
            IconSource::Preset(PresetIcon::Headphones),
        ));

        let ctx = egui::Context::default();
        let mut icon_cache = IconCache::new();
        let bounds = Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 200.0));
        let cells = calculate_grid_cells(bounds, 2, 2);
        let click_center = cells[1].center();

        // Frame 1: Pointer press at cell 1 center (with 1.0s timestamp)
        let mut raw_press = egui::RawInput::default();
        raw_press.screen_rect = Some(bounds);
        raw_press.time = Some(1.0);
        raw_press.events.push(egui::Event::PointerMoved(click_center));
        raw_press.events.push(egui::Event::PointerButton {
            pos: click_center,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        });
        let _ = ctx.run(raw_press, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let ev = render_button_grid(ui, bounds, &cfg, &audio, true, &mut icon_cache);
                assert_eq!(ev, GridEvent::None);
            });
        });

        // Frame 2: Pointer release 600ms later with 5px jitter (which previously failed under Sense::click_and_drag!)
        let release_pos = click_center + Vec2::new(4.0, 3.0);
        let mut raw_release = egui::RawInput::default();
        raw_release.screen_rect = Some(bounds);
        raw_release.time = Some(1.6);
        raw_release.events.push(egui::Event::PointerMoved(release_pos));
        raw_release.events.push(egui::Event::PointerButton {
            pos: release_pos,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        });
        let mut fired_event = GridEvent::None;
        let _ = ctx.run(raw_release, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                fired_event = render_button_grid(ui, bounds, &cfg, &audio, true, &mut icon_cache);
            });
        });

        assert_eq!(
            fired_event,
            GridEvent::SwitchDevice(1),
            "Clicking button #1 should fire GridEvent::SwitchDevice(1)"
        );
    }

    #[test]
    fn test_scroll_wheel_volume_and_middle_click_mute() {
        let audio = AudioEngine::new().expect("AudioEngine init");
        let mut cfg = AppConfig::default();
        cfg.window.columns = 2;
        cfg.buttons.push(ButtonConfig::new(
            "dev_0".to_string(),
            EndpointDirection::Output,
            "Device 0".to_string(),
            "#3B82F6".to_string(),
            IconSource::Preset(PresetIcon::Speakers),
        ));
        cfg.buttons.push(ButtonConfig::new(
            "dev_1".to_string(),
            EndpointDirection::Output,
            "Device 1".to_string(),
            "#10B981".to_string(),
            IconSource::Preset(PresetIcon::Headphones),
        ));

        let ctx = egui::Context::default();
        let mut icon_cache = IconCache::new();
        let bounds = Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 200.0));
        let cells = calculate_grid_cells(bounds, 2, 2);

        // 1. Scroll wheel up by 1 notch over button #1 -> +2% volume
        let hover_b1 = cells[1].center();
        let mut raw_scroll = egui::RawInput::default();
        raw_scroll.screen_rect = Some(bounds);
        raw_scroll.events.push(egui::Event::PointerMoved(hover_b1));
        raw_scroll.events.push(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Line,
            delta: Vec2::new(0.0, 1.0),
            modifiers: egui::Modifiers::NONE,
        });
        let mut scroll_event = GridEvent::None;
        let _ = ctx.run(raw_scroll, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                scroll_event = render_button_grid(ui, bounds, &cfg, &audio, true, &mut icon_cache);
            });
        });
        assert_eq!(
            scroll_event,
            GridEvent::AdjustVolume {
                button_idx: 1,
                delta_pct: 2
            }
        );

        // 2. Middle-click press & release over button #0 -> ToggleMuteDevice(0)
        let center_b0 = cells[0].center();
        let mut raw_mid_press = egui::RawInput::default();
        raw_mid_press.screen_rect = Some(bounds);
        raw_mid_press.events.push(egui::Event::PointerMoved(center_b0));
        raw_mid_press.events.push(egui::Event::PointerButton {
            pos: center_b0,
            button: PointerButton::Middle,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        });
        let _ = ctx.run(raw_mid_press, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let _ = render_button_grid(ui, bounds, &cfg, &audio, true, &mut icon_cache);
            });
        });

        let mut raw_mid_release = egui::RawInput::default();
        raw_mid_release.screen_rect = Some(bounds);
        raw_mid_release.events.push(egui::Event::PointerMoved(center_b0));
        raw_mid_release.events.push(egui::Event::PointerButton {
            pos: center_b0,
            button: PointerButton::Middle,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        });
        let mut mute_event = GridEvent::None;
        let _ = ctx.run(raw_mid_release, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                mute_event = render_button_grid(ui, bounds, &cfg, &audio, true, &mut icon_cache);
            });
        });
        assert_eq!(mute_event, GridEvent::ToggleMuteDevice(0));
    }
}
