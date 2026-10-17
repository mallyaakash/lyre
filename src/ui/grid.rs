use egui::{Id, PointerButton, Pos2, Rect, Ui, Vec2};

use crate::audio::AudioEngine;
use crate::config::{AppConfig, ButtonConfig};
use crate::ui::button_widget::{
    compute_ellipsis_menu_rect, draw_audio_button, ProportionalButtonMetrics,
};
use crate::ui::icons::IconCache;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GridEvent {
    None,
    SwitchDevice(usize),
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
    is_reordering: bool,
    secondary_press_pos: Option<Pos2>,
    secondary_dragged: bool,
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
        secondary_pressed,
        secondary_down,
        secondary_released,
        interact_pos,
        alt_held,
    ) = ui.input(|i| {
        (
            i.pointer.button_pressed(PointerButton::Primary),
            i.pointer.primary_down(),
            i.pointer.button_released(PointerButton::Primary),
            i.pointer.button_pressed(PointerButton::Secondary),
            i.pointer.secondary_down(),
            i.pointer.button_released(PointerButton::Secondary),
            i.pointer.interact_pos().or_else(|| i.pointer.latest_pos()),
            i.modifiers.alt,
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

    // 1. Handle Primary Press / Drag / Release deterministically
    if primary_pressed {
        if let Some(pos) = interact_pos {
            if available_rect.contains(pos) {
                if alt_held {
                    triggered_event = GridEvent::StartWindowDrag;
                } else if let Some(hit_idx) = cells.iter().position(|r| r.contains(pos)) {
                    tracker.primary_pressed_btn_id = Some(config.buttons[hit_idx].id.clone());
                    tracker.primary_press_pos = Some(pos);
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
            // Only enter drag-to-reorder mode if pointer moved more than 16 pixels from press origin
            if (curr_pos - origin).length() > 16.0 {
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
                    // Allow click if released inside the button cell (or within 16px of press origin)
                    let close_to_origin = tracker
                        .primary_press_pos
                        .map(|orig| (rel_pos - orig).length() <= 16.0)
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

    // 2. Handle Secondary (Right-Click) Press / Drag (Window Move) / Release (Context Menu)
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

    // 3. Draw all buttons
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

        draw_audio_button(
            ui,
            cell_rect,
            bullet_btn,
            is_active,
            is_muted,
            is_present,
            is_pressed_this,
            is_being_dragged,
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
}
