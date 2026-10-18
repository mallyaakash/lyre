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
use crate::tray::win32_window::compute_same_monitor_dialog_pos;
use crate::ui::grid::create_new_button_from_audio;
use crate::ui::icons::IconCache;
use crate::ui::theme::apply_theme;

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
    let font_id = FontId::new(12.5, FontFamily::Proportional);
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_string(), font_id.clone(), Color32::WHITE);
    let desired_size = Vec2::new(galley.size().x + 20.0, 26.0);
    let (rect, response) = ui.allocate_exact_size(desired_size, Sense::click());

    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }

    let (bg_fill, border_col, text_col) = match theme_mode {
        ThemeMode::Dark => {
            if accent_highlight {
                let bg = if response.hovered() {
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
                let bg = if response.hovered() {
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
                let bg = if response.hovered() {
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
            if accent_highlight || active_state {
                let bg = if response.hovered() {
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
                let bg = if response.hovered() {
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

    let rounding = Rounding::same(6.5);
    ui.painter().rect_filled(rect, rounding, bg_fill);
    ui.painter()
        .rect_stroke(rect, rounding, Stroke::new(1.0, border_col));
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        text,
        font_id,
        text_col,
    );

    response
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

/// Renders the top Edit/Configuration toolbar when Lyre is in Configuration / Edit Mode.
/// Returns `true` if any configuration property was modified (triggering auto-save).
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
        ui.spacing_mut().item_spacing = Vec2::new(6.0, 4.0);

        // Columns stepper
        ui.label(RichText::new("Columns").size(12.0).color(muted_text));
        if pill_button(ui, "−", false, false, theme)
            .on_hover_text("Fewer buttons per row")
            .clicked()
            && config.window.columns > 1
        {
            config.window.columns -= 1;
            changed = true;
        }
        ui.add_space(2.0);
        ui.label(
            RichText::new(format!("{}", config.window.columns))
                .size(13.0)
                .strong(),
        );
        ui.add_space(2.0);
        if pill_button(ui, "+", false, false, theme)
            .on_hover_text("More buttons per row")
            .clicked()
            && config.window.columns < 12
        {
            config.window.columns += 1;
            changed = true;
        }

        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);

        // Theme Mode toggle (Dark / Light)
        let theme_label = match config.window.theme_mode {
            ThemeMode::Dark => "Theme: Dark",
            ThemeMode::Light => "Theme: Light",
        };
        if pill_button(ui, theme_label, true, false, theme)
            .on_hover_text("Switch between Dark Mode (Matte Obsidian) and Light Mode")
            .clicked()
        {
            config.window.theme_mode = match config.window.theme_mode {
                ThemeMode::Dark => ThemeMode::Light,
                ThemeMode::Light => ThemeMode::Dark,
            };
            changed = true;
        }

        // Global Label quick-toggle
        let any_labels = config.buttons.iter().any(|b| b.show_label);
        let label_btn_text = if any_labels {
            "Labels: On"
        } else {
            "Labels: Off"
        };
        if pill_button(ui, label_btn_text, any_labels, false, theme)
            .on_hover_text("Toggle text titles on all buttons (Icon + Title vs Icon-Only)")
            .clicked()
        {
            let target = !any_labels;
            for b in &mut config.buttons {
                b.show_label = target;
            }
            changed = true;
        }

        // Mute All Outputs quick-toggle
        let out_muted = audio.are_outputs_muted();
        let mute_label = if out_muted { "Unmute All" } else { "Mute All" };
        if pill_button(ui, mute_label, out_muted, false, theme)
            .on_hover_text("Mute or unmute all active audio output devices")
            .clicked()
        {
            let _ = audio.toggle_mute_outputs();
        }

        // Add new button
        if pill_button(ui, "+ Add Button", false, true, theme)
            .on_hover_text("Bind a new audio input or output button (opens in an external dialog window)")
            .clicked()
        {
            let new_btn = create_new_button_from_audio(audio, config.buttons.len());
            config.buttons.push(new_btn);
            editor.selected_button_idx = Some(config.buttons.len() - 1);
            changed = true;
        }

        // Enter Minimalist Frameless Mode
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if pill_button(ui, "Minimalist Mode", false, true, theme)
                .on_hover_text(
                    "Strip all window bars and show strictly the button grid.\n• Right-click anywhere for context menu (Add Button, Mute All, etc.)\n• Drag buttons to reorder\n• Alt+Drag or Right-Drag to move window\n• Click ⋮ on any button to customize",
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

    let dialog_w = 456.0;
    let dialog_h = 600.0;
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
        .with_min_inner_size([400.0, 480.0])
        .with_always_on_top()
        .with_taskbar(false)
        .with_resizable(true);

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
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Accent Color:").size(12.5));
                        let mut rgb = parse_hex_color(&btn.color_hex).unwrap_or([0x3B, 0x82, 0xF6]);
                        if ui.color_edit_button_srgb(&mut rgb).changed() {
                            btn.color_hex = format_hex_color(rgb);
                            changed = true;
                        }
                        ui.monospace(RichText::new(&btn.color_hex).size(12.0));

                        ui.add_space(6.0);
                        for &swatch_hex in &DEFAULT_COLOR_PALETTE {
                            if let Some([r, g, b]) = parse_hex_color(swatch_hex) {
                                let c = Color32::from_rgb(r, g, b);
                                let is_sel = btn.color_hex.eq_ignore_ascii_case(swatch_hex);
                                let (rect, resp) =
                                    ui.allocate_exact_size(Vec2::new(21.0, 21.0), Sense::click());
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
                        RichText::new("CUSTOM IMAGE / TRANSPARENT ARTWORK")
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
                    if ui
                        .add_enabled(idx > 0, egui::Button::new("← Move Left"))
                        .clicked()
                    {
                        move_delta = -1;
                    }
                    if ui
                        .add_enabled(idx + 1 < total_buttons, egui::Button::new("Move Right →"))
                        .clicked()
                    {
                        move_delta = 1;
                    }
                    if ui
                        .button(
                            RichText::new("Delete Button")
                                .color(Color32::from_rgb(248, 113, 113)),
                        )
                        .clicked()
                    {
                        delete_requested = true;
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if pill_button(ui, "  Done  ", true, true, theme).clicked() {
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
/// (`.png`, `.webp`, `.svg`, `.ico`, `.jpg`), including a live thumbnail preview when active.
fn draw_prominent_upload_button(
    ui: &mut Ui,
    current_icon: &IconSource,
    accent: Color32,
    theme: ThemeMode,
    icon_cache: &mut IconCache,
) -> egui::Response {
    let desired = Vec2::new(ui.available_width(), 54.0);
    let (rect, resp) = ui.allocate_exact_size(desired, Sense::click());
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }

    let is_custom = matches!(current_icon, IconSource::CustomPath(_));
    let [ar, ag, ab, _] = accent.to_array();

    let (bg_fill, border_col, title_col, sub_col, badge_bg) = match theme {
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
                Color32::WHITE,
                Color32::from_rgb(182, 188, 204),
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
                Color32::from_rgb(18, 20, 26),
                Color32::from_rgb(75, 80, 94),
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

    let (primary_text, secondary_text) = if let IconSource::CustomPath(ref path) = current_icon {
        let fname = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("custom_image");
        (
            format!("Custom Image: {}", fname),
            "Click to upload a different image (.png, .webp, .svg, .ico, .jpg)".to_string(),
        )
    } else {
        (
            "Upload Custom Image / Artwork…".to_string(),
            "Supports transparent .PNG, .WEBP, .SVG, .ICO, .JPG".to_string(),
        )
    };

    ui.painter().text(
        Pos2::new(rect.left() + 56.0, rect.center().y - 7.5),
        Align2::LEFT_CENTER,
        primary_text,
        FontId::new(13.0, FontFamily::Proportional),
        title_col,
    );
    ui.painter().text(
        Pos2::new(rect.left() + 56.0, rect.center().y + 8.5),
        Align2::LEFT_CENTER,
        secondary_text,
        FontId::new(10.8, FontFamily::Proportional),
        sub_col,
    );

    // Right-side "Browse…" pill badge inside the upload card
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
