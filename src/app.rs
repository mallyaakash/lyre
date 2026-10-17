use std::time::{Duration, Instant};

use egui::{
    CentralPanel, Color32, Context, CursorIcon, Frame as EguiFrame, Key, Margin, Rect,
    ResizeDirection, Stroke, TopBottomPanel, ViewportCommand, WindowLevel,
};

use crate::audio::AudioEngine;
use crate::config::{self, AppConfig, ThemeMode};
use crate::tray::win32_window;
use crate::tray::{TrayCommand, TrayManager};
use crate::ui::grid::{render_button_grid, GridEvent};
use crate::ui::icons::IconCache;
use crate::ui::settings_panel::{
    render_button_editor_modal, render_top_config_bar, EditorState,
};
use crate::ui::theme::apply_theme;

pub struct LyreApp {
    audio: AudioEngine,
    config: AppConfig,
    tray: Option<TrayManager>,
    icon_cache: IconCache,
    editor: EditorState,
    hwnd: Option<isize>,
    initialized_window: bool,
    quit_requested: bool,
    dirty_config: bool,
    last_config_change: Instant,
    last_known_size: Option<[f32; 2]>,
    last_known_pos: Option<[f32; 2]>,
}

impl LyreApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        audio: AudioEngine,
        config: AppConfig,
        tray: Option<TrayManager>,
    ) -> Self {
        audio.set_egui_context(cc.egui_ctx.clone());
        apply_theme(&cc.egui_ctx, config.window.theme_mode);

        cc.egui_ctx
            .send_viewport_cmd(ViewportCommand::Decorations(!config.window.minimalist_mode));
        cc.egui_ctx.send_viewport_cmd(ViewportCommand::WindowLevel(
            if config.window.always_on_top {
                WindowLevel::AlwaysOnTop
            } else {
                WindowLevel::Normal
            },
        ));

        Self {
            audio,
            config,
            tray,
            icon_cache: IconCache::new(),
            editor: EditorState::default(),
            hwnd: None,
            initialized_window: false,
            quit_requested: false,
            dirty_config: false,
            last_config_change: Instant::now(),
            last_known_size: None,
            last_known_pos: None,
        }
    }

    fn mark_config_dirty(&mut self) {
        self.dirty_config = true;
        self.last_config_change = Instant::now();
    }

    fn sync_window_mode(&self, ctx: &Context) {
        apply_theme(ctx, self.config.window.theme_mode);

        ctx.send_viewport_cmd(ViewportCommand::Decorations(
            !self.config.window.minimalist_mode,
        ));
        let level = if self.config.window.always_on_top {
            WindowLevel::AlwaysOnTop
        } else {
            WindowLevel::Normal
        };
        ctx.send_viewport_cmd(ViewportCommand::WindowLevel(level));

        if let Some(hwnd) = self.hwnd {
            win32_window::apply_tool_window_style(hwnd);
            win32_window::apply_dark_title_bar_win32(
                hwnd,
                self.config.window.theme_mode == ThemeMode::Dark,
            );
            win32_window::set_always_on_top_win32(hwnd, self.config.window.always_on_top);
        }
        if let Some(ref tray) = self.tray {
            tray.set_always_on_top_checked(self.config.window.always_on_top);
        }
    }

    fn set_visibility(&mut self, _ctx: &Context, visible: bool) {
        self.config.window.visible = visible;
        if let Some(hwnd) = self.hwnd {
            win32_window::set_window_visible_win32(hwnd, visible);
            win32_window::apply_tool_window_style(hwnd);
            win32_window::apply_dark_title_bar_win32(
                hwnd,
                self.config.window.theme_mode == ThemeMode::Dark,
            );
        }
        self.mark_config_dirty();
    }

    fn switch_button_by_index(&mut self, btn_idx: usize) {
        let Some(btn) = self.config.buttons.get(btn_idx).cloned() else {
            return;
        };
        match self
            .audio
            .switch_button_device(&btn.device_id, &btn.title, btn.direction)
        {
            Ok(resolved_id) => {
                self.editor.status_message = None;
                if resolved_id != btn.device_id {
                    if let Some(slot) = self.config.buttons.get_mut(btn_idx) {
                        slot.device_id = resolved_id;
                    }
                    self.mark_config_dirty();
                }
            }
            Err(err) => {
                self.editor.status_message = Some(err);
            }
        }
    }

    fn process_tray_commands(&mut self, ctx: &Context) {
        let mut commands = Vec::new();
        if let Some(ref tray) = self.tray {
            if tray.is_quit_requested() {
                self.quit_requested = true;
            }
            while let Some(cmd) = tray.poll_command() {
                commands.push(cmd);
            }
        }

        for cmd in commands {
            match cmd {
                TrayCommand::SetVisibility(visible) => {
                    self.config.window.visible = visible;
                    if let Some(hwnd) = self.hwnd {
                        win32_window::apply_tool_window_style(hwnd);
                    }
                    self.mark_config_dirty();
                }
                TrayCommand::SetAlwaysOnTop(top) => {
                    self.config.window.always_on_top = top;
                    self.sync_window_mode(ctx);
                    self.mark_config_dirty();
                }
                TrayCommand::OpenConfigMode => {
                    self.config.window.minimalist_mode = false;
                    self.config.window.visible = true;
                    self.sync_window_mode(ctx);
                    if let Some(hwnd) = self.hwnd {
                        win32_window::set_window_visible_win32(hwnd, true);
                    }
                    ctx.send_viewport_cmd(ViewportCommand::Focus);
                    self.mark_config_dirty();
                }
                TrayCommand::Quit => {
                    self.config.window.visible = true;
                    let _ = config::save_config(&self.config);
                    self.quit_requested = true;
                    ctx.send_viewport_cmd(ViewportCommand::Close);
                }
                TrayCommand::ContextSwitchButton(idx) => {
                    self.switch_button_by_index(idx);
                }
                TrayCommand::ContextEditButton(idx) => {
                    if idx < self.config.buttons.len() {
                        self.editor.selected_button_idx = Some(idx);
                    }
                }
                TrayCommand::ContextDuplicateButton(idx) => {
                    if let Some(mut cloned) = self.config.buttons.get(idx).cloned() {
                        cloned.id = uuid::Uuid::new_v4().to_string();
                        cloned.badge_enabled = true;
                        cloned.badge_number = cloned.badge_number.saturating_add(1);
                        let insert_at = (idx + 1).min(self.config.buttons.len());
                        self.config.buttons.insert(insert_at, cloned);
                        self.mark_config_dirty();
                    }
                }
                TrayCommand::ContextDeleteButton(idx) => {
                    if idx < self.config.buttons.len() {
                        self.config.buttons.remove(idx);
                        if self.editor.selected_button_idx == Some(idx) {
                            self.editor.selected_button_idx = None;
                        }
                        self.mark_config_dirty();
                    }
                }
                TrayCommand::ContextAddButton => {
                    let new_btn = crate::ui::grid::create_new_button_from_audio(
                        &self.audio,
                        self.config.buttons.len(),
                    );
                    self.config.buttons.push(new_btn);
                    self.editor.selected_button_idx = Some(self.config.buttons.len() - 1);
                    self.mark_config_dirty();
                }
                TrayCommand::ContextToggleMuteOutputs => {
                    if let Err(err) = self.audio.toggle_mute_outputs() {
                        self.editor.status_message = Some(err);
                    }
                }
                TrayCommand::ContextToggleMuteInputs => {
                    if let Err(err) = self.audio.toggle_mute_inputs() {
                        self.editor.status_message = Some(err);
                    }
                }
                TrayCommand::ContextToggleMinimalist => {
                    self.config.window.minimalist_mode = !self.config.window.minimalist_mode;
                    self.sync_window_mode(ctx);
                    self.mark_config_dirty();
                }
                TrayCommand::ContextToggleTheme => {
                    self.config.window.theme_mode = match self.config.window.theme_mode {
                        ThemeMode::Dark => ThemeMode::Light,
                        ThemeMode::Light => ThemeMode::Dark,
                    };
                    self.sync_window_mode(ctx);
                    self.mark_config_dirty();
                }
                TrayCommand::ContextToggleLabels => {
                    let any_shown = self.config.buttons.iter().any(|b| b.show_label);
                    let next = !any_shown;
                    for btn in &mut self.config.buttons {
                        btn.show_label = next;
                    }
                    self.mark_config_dirty();
                }
            }
        }
    }

    /// Detects 8-way edge/corner resize interactions when operating in Minimalist Frameless Mode.
    fn handle_frameless_edge_resize(&self, ctx: &Context, viewport_rect: Rect) -> bool {
        if !self.config.window.minimalist_mode {
            return false;
        }

        let border = 6.0;
        let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) else {
            return false;
        };

        let left = (pos.x - viewport_rect.left()).abs() <= border;
        let right = (viewport_rect.right() - pos.x).abs() <= border;
        let top = (pos.y - viewport_rect.top()).abs() <= border;
        let bottom = (viewport_rect.bottom() - pos.y).abs() <= border;

        let dir = match (left, right, top, bottom) {
            (true, _, true, _) => Some((ResizeDirection::NorthWest, CursorIcon::ResizeNwSe)),
            (_, true, true, _) => Some((ResizeDirection::NorthEast, CursorIcon::ResizeNeSw)),
            (true, _, _, true) => Some((ResizeDirection::SouthWest, CursorIcon::ResizeNeSw)),
            (_, true, _, true) => Some((ResizeDirection::SouthEast, CursorIcon::ResizeNwSe)),
            (true, _, _, _) => Some((ResizeDirection::West, CursorIcon::ResizeHorizontal)),
            (_, true, _, _) => Some((ResizeDirection::East, CursorIcon::ResizeHorizontal)),
            (_, _, true, _) => Some((ResizeDirection::North, CursorIcon::ResizeVertical)),
            (_, _, _, true) => Some((ResizeDirection::South, CursorIcon::ResizeVertical)),
            _ => None,
        };

        if let Some((resize_dir, cursor)) = dir {
            ctx.set_cursor_icon(cursor);
            if ctx.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary)) {
                ctx.send_viewport_cmd(ViewportCommand::BeginResize(resize_dir));
            }
            true
        } else {
            false
        }
    }

    fn track_viewport_geometry(&mut self, ctx: &Context) {
        let (inner_rect, outer_rect) = ctx.input(|i| {
            let vp = i.viewport();
            (vp.inner_rect, vp.outer_rect)
        });

        if let Some(inner) = inner_rect {
            let sz = [inner.width(), inner.height()];
            if sz[0] >= 80.0 && sz[1] >= 50.0 {
                let changed = self
                    .last_known_size
                    .map(|prev| (prev[0] - sz[0]).abs() > 1.0 || (prev[1] - sz[1]).abs() > 1.0)
                    .unwrap_or(true);
                if changed {
                    self.last_known_size = Some(sz);
                    self.config.window.width = sz[0];
                    self.config.window.height = sz[1];
                    self.mark_config_dirty();
                }
            }
        }

        if let Some(outer) = outer_rect {
            let pos = [outer.min.x, outer.min.y];
            if pos[0] > -10000.0 && pos[1] > -10000.0 {
                let changed = self
                    .last_known_pos
                    .map(|prev| (prev[0] - pos[0]).abs() > 1.0 || (prev[1] - pos[1]).abs() > 1.0)
                    .unwrap_or(true);
                if changed {
                    self.last_known_pos = Some(pos);
                    self.config.window.x = Some(pos[0]);
                    self.config.window.y = Some(pos[1]);
                    self.mark_config_dirty();
                }
            }
        }
    }
}

impl eframe::App for LyreApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        match self.config.window.theme_mode {
            ThemeMode::Dark => [17.0 / 255.0, 17.0 / 255.0, 19.0 / 255.0, 1.0],
            ThemeMode::Light => [244.0 / 255.0, 244.0 / 255.0, 246.0 / 255.0, 1.0],
        }
    }

    fn update(&mut self, ctx: &Context, frame: &mut eframe::Frame) {
        // 1. One-time Win32 HWND setup: apply WS_EX_TOOLWINDOW & DWM immersive dark mode
        if !self.initialized_window {
            apply_theme(ctx, self.config.window.theme_mode);
            if let Some(hwnd) = win32_window::extract_hwnd(frame) {
                self.hwnd = Some(hwnd);
                win32_window::apply_tool_window_style(hwnd);
                win32_window::apply_dark_title_bar_win32(
                    hwnd,
                    self.config.window.theme_mode == ThemeMode::Dark,
                );
                win32_window::set_always_on_top_win32(hwnd, self.config.window.always_on_top);
                if let Some(ref tray) = self.tray {
                    tray.bind_window_and_context(hwnd, ctx.clone());
                }
            }
            self.initialized_window = true;
        }

        // 2. Poll real-time COM IMMNotificationClient updates & System Tray commands first
        self.audio.poll_updates();
        self.process_tray_commands(ctx);

        // 3. Intercept window close button (X): hide to System Tray unless Quit was explicitly requested
        if ctx.input(|i| i.viewport().close_requested()) {
            if self.quit_requested {
                let _ = config::save_config(&self.config);
                return;
            } else {
                ctx.send_viewport_cmd(ViewportCommand::CancelClose);
                self.set_visibility(ctx, false);
            }
        }

        // 4. Keyboard shortcuts:
        //    - F2 or Ctrl+Comma: Toggle Minimalist Mode <-> Configuration/Edit Mode
        //    - Ctrl+Q: Quit Lyre
        let toggle_mode_shortcut = ctx.input(|i| {
            i.key_pressed(Key::F2) || (i.modifiers.ctrl && i.key_pressed(Key::Comma))
        });
        if toggle_mode_shortcut {
            self.config.window.minimalist_mode = !self.config.window.minimalist_mode;
            self.sync_window_mode(ctx);
            self.mark_config_dirty();
        }
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(Key::Q)) {
            self.config.window.visible = true;
            let _ = config::save_config(&self.config);
            self.quit_requested = true;
            ctx.send_viewport_cmd(ViewportCommand::Close);
            return;
        }

        // 5. Track window position and size changes
        self.track_viewport_geometry(ctx);

        let edit_mode = !self.config.window.minimalist_mode;

        // 6. Top Configuration Toolbar (only shown when in Configuration / Edit Mode)
        if edit_mode {
            let mut enter_minimalist = false;
            let (bar_bg, bar_border) = match self.config.window.theme_mode {
                ThemeMode::Dark => (
                    Color32::from_rgb(20, 20, 23),
                    Color32::from_rgb(38, 38, 44),
                ),
                ThemeMode::Light => (
                    Color32::from_rgb(246, 246, 249),
                    Color32::from_rgb(220, 220, 226),
                ),
            };

            TopBottomPanel::top("lyre_config_top_bar")
                .frame(
                    EguiFrame::none()
                        .fill(bar_bg)
                        .stroke(Stroke::new(1.0, bar_border))
                        .inner_margin(Margin::symmetric(10.0, 7.0)),
                )
                .show(ctx, |ui| {
                    let prev_theme = self.config.window.theme_mode;
                    let prev_top = self.config.window.always_on_top;
                    if render_top_config_bar(
                        ui,
                        &mut self.config,
                        &mut self.audio,
                        &mut self.editor,
                        &mut enter_minimalist,
                    ) {
                        if self.config.window.theme_mode != prev_theme
                            || self.config.window.always_on_top != prev_top
                        {
                            self.sync_window_mode(ctx);
                        }
                        self.mark_config_dirty();
                    }
                });

            if enter_minimalist {
                self.editor.selected_button_idx = None;
                self.config.window.minimalist_mode = true;
                self.sync_window_mode(ctx);
                self.mark_config_dirty();
            }
        }

        // 7. Central Responsive Button Grid
        let panel_bg = match self.config.window.theme_mode {
            ThemeMode::Dark => Color32::from_rgb(17, 17, 19),
            ThemeMode::Light => Color32::from_rgb(244, 244, 246),
        };

        CentralPanel::default()
            .frame(EguiFrame::none().fill(panel_bg).inner_margin(Margin::same(2.0)))
            .show(ctx, |ui| {
                let viewport_rect = ui.max_rect();
                let in_resize_zone = self.handle_frameless_edge_resize(ctx, viewport_rect);

                let bg_event = render_button_grid(
                    ui,
                    viewport_rect,
                    &self.config,
                    &self.audio,
                    !self.config.window.minimalist_mode,
                    &mut self.icon_cache,
                );

                if !in_resize_zone {
                    match bg_event {
                        GridEvent::SwitchDevice(btn_idx) => {
                            self.switch_button_by_index(btn_idx);
                        }
                        GridEvent::EditButton(idx) => {
                            self.editor.selected_button_idx = Some(idx);
                        }
                        GridEvent::MoveButton { from, to } => {
                            if from < self.config.buttons.len() && to < self.config.buttons.len() {
                                let item = self.config.buttons.remove(from);
                                self.config.buttons.insert(to, item);
                                if self.editor.selected_button_idx == Some(from) {
                                    self.editor.selected_button_idx = Some(to);
                                }
                                self.mark_config_dirty();
                            }
                        }
                        GridEvent::OpenContextMenu { button_idx } => {
                            if let (Some(hwnd), Some(ref mut tray)) = (self.hwnd, &mut self.tray) {
                                let target_tuple = button_idx.and_then(|idx| {
                                    self.config
                                        .buttons
                                        .get(idx)
                                        .map(|b| (idx, b.title.as_str()))
                                });
                                tray.show_window_context_menu(
                                    hwnd,
                                    target_tuple,
                                    self.audio.are_outputs_muted(),
                                    self.audio.are_inputs_muted(),
                                    self.config.window.minimalist_mode,
                                    self.config.window.theme_mode == ThemeMode::Dark,
                                    self.config.buttons.iter().any(|b| b.show_label),
                                    self.config.window.always_on_top,
                                );
                            }
                        }
                        GridEvent::StartWindowDrag => {
                            ctx.send_viewport_cmd(ViewportCommand::StartDrag);
                        }
                        GridEvent::AddFirstButton => {
                            let new_btn = crate::ui::grid::create_new_button_from_audio(
                                &self.audio,
                                self.config.buttons.len(),
                            );
                            self.config.buttons.push(new_btn);
                            self.editor.selected_button_idx = Some(self.config.buttons.len() - 1);
                            self.mark_config_dirty();
                        }
                        GridEvent::None => {
                            let pointer_drag = ui.input(|i| {
                                i.pointer.is_decidedly_dragging()
                                    && ((i.pointer.primary_down() && i.modifiers.alt)
                                        || i.pointer.secondary_down())
                            });
                            if pointer_drag && self.editor.selected_button_idx.is_none() {
                                ctx.send_viewport_cmd(ViewportCommand::StartDrag);
                            }
                        }
                    }
                }
            });

        // 8. Per-button Customization Modal (opens on the same monitor as main_hwnd)
        if render_button_editor_modal(
            ctx,
            self.hwnd,
            &mut self.config,
            &mut self.audio,
            &mut self.editor,
            &mut self.icon_cache,
        ) {
            self.mark_config_dirty();
        }

        // 9. Debounced auto-save to %APPDATA%/Lyre/config.json
        if self.dirty_config
            && self.last_config_change.elapsed() >= Duration::from_millis(250)
        {
            let _ = config::save_config(&self.config);
            self.dirty_config = false;
        } else if self.dirty_config {
            ctx.request_repaint_after(Duration::from_millis(260));
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        let _ = config::save_config(&self.config);
    }
}
