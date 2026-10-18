# Lyre — Agent Engineering Protocol & Checkpoint Tracker (`agent.md`)

## Core Operating Rules for the Agent
1. **Always Check & Update This File:** Before starting a new phase or sub-task, and immediately after completing one, read and update `agent.md` (and the companion `task.md` artifact) to mark completed items (`[x]`) and log the current checkpoint status.
2. **Never Skip Requirements:** Every single item listed under the specification checkpoints below must be implemented and verified before declaring a phase complete.
3. **Compile & Test at Every Checkpoint:** Run `cargo check` / `cargo test` at the end of every phase checkpoint before advancing to the next phase.

---

## Master Tasklist & Checkpoints

### Checkpoint 0: Project Scaffolding & Dependencies
- [x] **0.1** Create `Cargo.toml` with all required dependencies (`windows`, `windows-core`, `eframe`, `egui`, `egui_extras`, `resvg`, `image`, `tray-icon`, `muda`, `serde`, `serde_json`, `directories`, `rfd`, `raw-window-handle`, `uuid`).
- [x] **0.2** Create `build.rs` (optional Windows icon embedding from `logo.png`) and modular directory tree (`src/audio`, `src/config`, `src/tray`, `src/ui`).

### Checkpoint 1: Core Audio Engine (`src/audio/`)
- [x] **1.1** Implement `src/audio/policy_config.rs`:
  - [x] Define `CLSID_PolicyConfigClient` (`{870af99c-171d-4f9e-af0d-e63df40c2bc9}`).
  - [x] Define `IPolicyConfig` (`{f8679f50-850a-41cf-9c72-430f290290c8}`) with exact 12-method vtable layout (`SetDefaultEndpoint` at slot 13).
  - [x] Define `IPolicyConfigVista` (`{568b9108-44bf-40b4-9006-86afe5b5a620}`) fallback interface.
  - [x] Implement `set_default_endpoint(device_id: &str)` switching all three roles (`eConsole`, `eMultimedia`, `eCommunications`) simultaneously.
- [x] **1.2** Implement `src/audio/enumerator.rs`:
  - [x] Use `IMMDeviceEnumerator` and `IMMDeviceCollection` to enumerate active `eRender` (Output) and `eCapture` (Input) endpoints (`DEVICE_STATE_ACTIVE`).
  - [x] Retrieve persistent Endpoint IDs (`IMMDevice::GetId`) and friendly names (`PKEY_Device_FriendlyName`).
  - [x] Query current default Output and Input Endpoint IDs via `GetDefaultAudioEndpoint`.
- [x] **1.3** Implement `src/audio/notifier.rs`:
  - [x] Implement `IMMNotificationClient` COM interface (`OnDefaultDeviceChanged`, `OnDeviceStateChanged`, `OnDeviceAdded`, `OnDeviceRemoved`, `OnPropertyValueChanged`).
  - [x] Trigger atomic dirty flag + immediate GUI wakeup (`egui::Context::request_repaint()`) when external audio changes happen.
- [x] **1.4** Implement `src/audio/mod.rs`:
  - [x] Provide `AudioEngine` wrapper with thread-safe state refresh, device lookup, and unit tests.
- [x] **1.5** **Verification Gate 1:** `cargo test` passes on audio module & COM GUID/enumeration checks.

### Checkpoint 2: Configuration & Persistence (`src/config/`)
- [x] **2.1** Implement `src/config/schema.rs`:
  - [x] `DeviceKind` (`Output`, `Input`).
  - [x] `ThemeMode` (`Light`, `Dark`).
  - [x] `PresetIcon` enum (`Speakers`, `Headphones`, `IEMs`, `StudioMonitors`, `Microphone`, `HeadsetMic`, `DacAmp`).
  - [x] `IconSource` (`Preset(PresetIcon)`, `CustomPath(PathBuf)`).
  - [x] `ButtonConfig` (`id`, `device_id`, `device_kind`, `title`, `color_hex`, `icon`, `show_label`, `badge_enabled`, `badge_number`).
  - [x] `WindowState` (`x`, `y`, `width`, `height`, `minimalist_mode`, `always_on_top`, `columns`, `theme_mode`, `visible`).
  - [x] `AppConfig` combining `WindowState` and `Vec<ButtonConfig>`.
- [x] **2.2** Implement `src/config/mod.rs`:
  - [x] Resolve `%APPDATA%/Lyre/config.json`.
  - [x] Load configuration on startup, or auto-seed initial buttons from currently detected Windows Output & Input devices on first run.
  - [x] Atomic save (`config.json.tmp` -> `config.json`) on state/config changes.
- [x] **2.3** **Verification Gate 2:** Unit tests for config serialization/deserialization and color hex parsing pass.

### Checkpoint 3: Dynamic Grid, Proportional Scaling & Button Customization (`src/ui/`)
- [x] **3.1** Implement `src/ui/icons.rs`:
  - [x] Clean, scalable built-in SVG definitions for all 7 required presets:
    1. Speakers
    2. Headphones
    3. IEMs
    4. Studio Monitors
    5. Microphone
    6. Headset Mic
    7. DAC/Amp
  - [x] Rasterization & caching pipeline using `resvg` and `image` for both built-in SVGs (with dynamic tinting) and custom local user media (`.png`, `.svg`, `.ico`, `.jpg`).
- [x] **3.2** Implement `src/ui/theme.rs`:
  - [x] Light Mode styling (user-selectable button fill color, contrasting icon/text, active highlight ring).
  - [x] Dark Mode styling (dark-toned button body, user-selectable colored outline/border, subtle glow/accent when active).
- [x] **3.3** Implement `src/ui/button_widget.rs`:
  - [x] Proportional scaling math relative to button cell size $L$: corner radius, border width, centered icon sizing, optional text label (`Icon + Text Title` vs `Icon-Only`), and circular corner **Number Badge** (`1`, `2`, `3`, etc.).
  - [x] Active state visual highlight for currently active Default Output and Default Input buttons.
  - [x] Right-click / Edit button context action to quickly edit or bind a button.
- [x] **3.4** Implement `src/ui/grid.rs`:
  - [x] Dynamic $N$-column responsive grid calculating exact cell bounds and proportional scale factors as the window is resized.
- [x] **3.5** Implement `src/ui/settings_panel.rs`:
  - [x] Full Configuration / Edit Mode UI:
    - Adjust columns ($N$ buttons per row).
    - Toggle Light Mode / Dark Mode.
    - Toggle Minimalist Frameless Mode & Always on Top.
    - Add, edit, reorder, or delete buttons.
    - Per-button editor: select bound Windows Audio Endpoint (Output or Input), custom title, color picker / hex input, icon preset selector or `rfd` custom image/icon file browser (`.png`, `.svg`, `.ico`, `.jpg`), toggle text label visibility, toggle & set number badge (`1`, `2`, `3`...).
- [x] **3.6** **Verification Gate 3:** `cargo check` & layout math unit tests pass.

### Checkpoint 4: System Tray & Minimalist "Buttons-Only" Mode (`src/tray/` & `src/app.rs`)
- [x] **4.1** Implement `src/tray/win32_window.rs`:
  - [x] Extract Win32 `HWND` and apply `WS_EX_TOOLWINDOW` (and strip `WS_EX_APPWINDOW`) to hide the app from the Windows Taskbar.
  - [x] Implement reliable Win32 show/hide, foreground activation, and `WM_NULL` event-loop wakeup when hidden.
- [x] **4.2** Implement `src/tray/mod.rs`:
  - [x] Build system tray icon from `logo.png` using `tray-icon`.
  - [x] Build native right-click context menu using `muda`:
    - `Show / Hide Buttons` (plus left-click tray icon toggle).
    - `Always on Top` (checkable toggle).
    - `Open Configuration / Edit Mode`.
    - `Quit Lyre`.
- [x] **4.3** Implement `src/app.rs` & `src/main.rs`:
  - [x] Synchronize tray events, `IMMNotificationClient` events, and `eframe` viewport state.
  - [x] Strip all OS window decorations in Minimalist Mode (rendering strictly the tight grid of buttons).
  - [x] Implement `Alt + Left-Drag` and `Right-Click Drag` to move the frameless window.
  - [x] Implement frameless window edge/corner resize hit-zones (`ViewportCommand::BeginResize`) so proportional scaling works seamlessly in Minimalist Mode.
  - [x] Debounced auto-save of window coordinates, dimensions, and settings to `%APPDATA%/Lyre/config.json`.
- [x] **4.4** **Verification Gate 4:** Full build (`cargo build --release`), unit tests (`cargo test`), and live runtime verification of Lyre.

### Checkpoint 5: UI Polish, Logo Cropping, Drag-to-Reorder, Dark Grey Theme & System Tray Fix
- [x] **5.1** Tightly crop `logo.png` to remove outer whitespace so the Lyre emblem is large and legible in the window title bar and system tray.
- [x] **5.2** Fix System Tray (`src/tray/mod.rs` & `src/tray/win32_window.rs`):
  - [x] Retain `muda::MenuItem` handles inside `TrayManager` so menu items are never dropped prematurely.
  - [x] Execute `ShowWindow(SW_SHOW)` / `SW_HIDE` and Quit actions directly from the tray event handler thread so `Show / Hide Buttons`, `Open Configuration / Edit Mode`, `Always on Top`, and `Quit Lyre` work reliably even when the window is hidden (`SW_HIDE`).
- [x] **5.3** True Neutral Dark Grey Theme & Full Dark Mode (`src/ui/theme.rs` & `src/tray/win32_window.rs`):
  - [x] Fix `apply_theme` bug where `ctx.set_style` overwrote `ctx.set_visuals` (which caused the top toolbar and modals to render in light mode).
  - [x] Replace the bluish dark palette with a sleek neutral dark grey palette (`#121214` background, `#1C1C1F` cards, `#28282D` hover) and apply Win32 `DWMWA_USE_IMMERSIVE_DARK_MODE` to the native window frame.
- [x] **5.4** Three Vertical Ellipses (`⋮`) Edit Button & Drag-to-Reorder (`src/ui/button_widget.rs` & `src/ui/grid.rs`):
  - [x] Draw three vertical vector dots (`⋮`) in the top-right corner of each button using `Painter::circle_filled` (eliminating the missing font glyph `□`).
  - [x] Make buttons draggable via left-click drag to easily rearrange/reorder buttons in the grid with live visual feedback.
- [x] **5.5** Modern Minimalist UI Polish (`src/ui/settings_panel.rs`, `src/ui/button_widget.rs`):
  - [x] Replace basic default `egui` toolbar buttons and broken unicode emojis with custom-painted pill controls, clean vector icons, and refined card typography.
- [x] **5.6** **Verification Gate 5:** Compile release build (`cargo build --release`), pass all unit tests (`cargo test`), and verify live execution.

### Checkpoint 6: Priority Device Switching Fix, External Dialog Viewport & Right-Click Context Menu
- [x] **6.1 [PRIORITY]** Fix & Verify Default Audio Device Switching (`src/audio/policy_config.rs`, `src/ui/button_widget.rs`, `src/ui/grid.rs`):
  - [x] Fix `Sense::click_and_drag` click swallowing in `button_widget.rs` (where `egui` marks any slight pointer movement or >300ms hold during press as a drag and suppresses `response.clicked()`, preventing `SwitchDevice` from firing) by replacing it with a deterministic `GridPointerTracker` state machine.
  - [x] Verify `IPolicyConfig::SetDefaultEndpoint` on Windows 11 with a live round-trip automated test (`test_audio_engine_initialization_and_switching`) that switches the active output device, queries `GetDefaultAudioEndpoint` from Windows to confirm the switch took effect, and restores the original default device.
  - [x] Add fallback device matching by friendly name in case a saved button's endpoint GUID changed across reboots/driver resets, plus a 35ms follow-up re-assertion to prevent background virtual audio routers (e.g., SteelSeries Sonar) from hijacking user switches.
- [x] **6.2** Independent OS Window for Add / Customize Button Dialog (`src/ui/settings_panel.rs` & `src/app.rs`):
  - [x] Render the Add / Customize Button dialog using `ctx.show_viewport_immediate` (a separate native OS window) so the entire dialog box is drawn outside the main app window and remains 100% visible even when the main button window is resized very small.
- [x] **6.3** Right-Click Context Menu & Mute All Audio (`src/audio/volume.rs`, `src/tray/mod.rs`, `src/app.rs`):
  - [x] Implement `IAudioEndpointVolume` COM control in `src/audio/volume.rs` to support **Mute All Outputs / Unmute All Outputs** and **Mute All Microphones**.
  - [x] Provide a native Win32 popup menu (via `muda::ContextMenu::show_context_menu_for_hwnd` so it can also extend outside a tiny main window!) triggered on right-clicking either the window background or any button:
    - When right-clicking a button: `Switch to "<Device>"`, `Customize "<Device>"…`, `Duplicate Button`, `Delete Button`, separator, plus global actions.
    - Global actions (on background or button): `Add New Audio Button…`, `Mute All Outputs (Speakers / Headphones)`, `Mute All Microphones`, `Minimalist Frameless Mode`, `Dark Mode`, `Show Text Labels`, `Always on Top`, `Hide Window to System Tray`, `Quit Lyre`.
- [x] **6.4** **Verification Gate 6:** Run automated live device-switching verification test (`cargo test -- --test-threads=1 --nocapture`), build release binary (`cargo build --release`), and verify live execution.

### Checkpoint 7: Hybrid SteelSeries Sonar Local API Bridge & Automatic Windows Fallback
- [x] **7.1** Probe and verify live SteelSeries GG / Sonar localhost REST endpoints (`C:\ProgramData\SteelSeries\SteelSeries Engine 3\coreProps.json` -> `/subApps` -> `sonar` `webServerAddress`, `/mode`, `/classicRedirections`, `/streamRedirections`, `/audioDevices`).
- [x] **7.2** Implement `src/audio/sonar.rs` (Pure-Rust Sonar Local API Client with zero external heavy dependencies):
  - [x] Discover `webServerAddress` dynamically from `coreProps.json` (`ggEncryptedAddress`).
  - [x] Support both Classic mode (`/classicRedirections/{channel}/deviceId/{device_id}` for `game`, `chat`, `media`, `aux`, `mic`) and Streamer mode (`/streamRedirections/...`).
  - [x] Query current Sonar active physical redirection device IDs (`game` / `mic`) so Lyre highlights the physical device currently active inside Sonar.
  - [x] Prefer physical devices over `SteelSeries Sonar - *` virtual endpoints when adding buttons, and ensure 100% graceful fallback to standard Windows `IPolicyConfig` when Sonar is not installed or not running (e.g. on a laptop).
- [x] **7.3** Integrate `sonar.rs` into `AudioEngine` (`src/audio/mod.rs`) and `ui/grid.rs` active-state highlighting:
  - [x] When switching an Output button, redirect Sonar's output channels (`game`, `chat`, `media`, `aux`) to the target device (or switch Windows default output directly when Sonar isn't running).
  - [x] When switching an Input button, redirect Sonar's input channel (`mic`) to the target microphone (or switch Windows default input directly when Sonar isn't running).
  - [x] Track both the Windows default endpoint ID and Sonar's active redirected physical device ID so the active button highlight in Lyre reflects the real physical device in use.
- [x] **7.4** **Verification Gate 7:** Run automated live Sonar redirection + Windows switching round-trip test (`cargo test -- --test-threads=1 --nocapture`), build release binary (`cargo build --release`), and launch.

### Checkpoint 8: Studio LED Indicators, Glassy Buttons, Modern Customize Dialog, WebP/Transparency, Multi-Monitor Positioning & Git Repo
- [x] **8.1** Enable `.webp` and full RGBA transparency support for custom uploaded images (`Cargo.toml`, `src/ui/icons.rs`, `src/ui/button_widget.rs`, `src/ui/settings_panel.rs`).
- [x] **8.2** Implement Studio Hardware Rectangular LED Light & Glassy Button Effect (`src/ui/button_widget.rs`, `src/audio/volume.rs`, `src/audio/mod.rs`):
  - [x] Studio rectangular LED light on every button: **Green glowing LED** when active/in-use, **Red glowing LED** when muted, **Empty recessed grey rectangular LED** when unused.
  - [x] Multi-layered frosted glass / acrylic button aesthetic (top specular glass sheen, crisp top-edge rim highlight, translucent layered body, and subtle accent border).
- [x] **8.3** Redesign Customize Button Dialog UI/UX & Same-Monitor Positioning (`src/ui/settings_panel.rs`, `src/tray/win32_window.rs`, `src/app.rs`):
  - [x] Position the Customize Button viewport on the **same monitor** as the main app window (clamped to that monitor's work area via Win32 `MonitorFromWindow` / `GetMonitorInfoW`).
  - [x] Redesign the Customize Button window with modern section cards, visual SVG icon tiles for the 7 presets, and a prominent, distinct **"Upload Custom Image (.png, .webp, .svg, .ico, .jpg)"** banner button with live image preview.
- [x] **8.4** Initialize Git repository (`git init`, `.gitignore`, initial commit) and run **Verification Gate 8** (`cargo test`, `cargo build --release`, and launch).

### Checkpoint 9: Linear / Raycast Matte Obsidian UI/UX Redesign
- [x] **9.1** Remove all two-tone half-rect reflection bands (`upper_glass_rect`, `sheen_rect`) across buttons, pills, color swatches, preset tiles, and upload cards (`src/ui/button_widget.rs`, `src/ui/settings_panel.rs`).
- [x] **9.2** Implement **Linear / Raycast Matte Obsidian** palette & styling (`src/ui/theme.rs`, `src/ui/button_widget.rs`, `src/app.rs`):
  - [x] Deep carbon canvas (`#0D0E11` in Dark Mode; `#F4F5F8` in Light Mode).
  - [x] Uniform matte button cards (`#15171C` idle, `#1C1F26` hover, uniform subtle 10% full-card accent tint when active) with crisp `1px` hairline borders (`#252830` idle, smooth accent border when hovered/active) and a single subtle `1px` top-edge rim highlight.
  - [x] Subtle rounded squircle icon well behind preset icons (or clean un-clipped showcase for custom transparent artwork) + refined typography.
  - [x] Flush studio rectangular LED indicator (`#22C55E` green when active, `#EF4444` red when muted, `#1E2129` recessed matte slot when unused).
- [x] **9.3** Refine Top Config Bar & Customize Button Dialog (`src/ui/settings_panel.rs`) to match the Linear / Raycast Matte Obsidian design language (uniform matte section cards, clean hairline borders, custom-painted toggle switches instead of default checkboxes, and zero two-tone bands).
- [x] **9.4** **Verification Gate 9:** Run `cargo test`, build release binary (`cargo build --release`), commit to Git, and launch.

### Checkpoint 10: Transparent `logo_bg.png` Taskbar/Tray Icon & Linear / Raycast Matte Obsidian Right-Click Context Window
- [x] **10.1** Embed `G:\Projects\Lyre\logo_bg.png` (transparent background, auto-cropped to centered square canvas) as the System Tray and Taskbar/Window icon (`src/tray/mod.rs`).
- [x] **10.2** Replace the classic OS right-click menu with a custom frameless **Linear / Raycast Matte Obsidian** right-click context window (`render_context_menu_viewport` in `src/ui/settings_panel.rs`, `compute_same_monitor_cursor_menu_pos` in `src/tray/win32_window.rs`, and `src/app.rs`), plus enable Win32 dark mode (`uxtheme.dll`) for the System Tray menu.
- [x] **10.3** **Verification Gate 10:** Run `cargo test`, build release binary (`cargo build --release`), commit to Git, and launch.

### Checkpoint 11: +50% Button Image Size, `Bitcount_Prop_Single` Font, Icon Toolbar with Centered Columns & Mic Mute, Title Bar Logo (`logo.png`), Vector Arrow Icons & Hard Minimum Dialog Size
- [x] **11.1** Increase button image size by 50% (`0.585 * unit_l` with label, `0.72 * unit_l` icon-only), reduce label font size (`0.095 * unit_l`) to fit cleanly, and register `Bitcount_Prop_Single` (`assets/fonts/Bitcount_Prop_Single/static/BitcountPropSingle-Medium.ttf`) for button labels, badges, and tags (`src/ui/theme.rs`, `src/ui/button_widget.rs`).
- [x] **11.2** Add custom vector icons (`ToolbarIcon`) to all top toolbar buttons, move the `Columns` selector to the middle of the toolbar, and add a `Mute Mic` button alongside `Mute Out` differentiated by Studio Microphone vs Speaker vector icons (`src/ui/settings_panel.rs`).
- [x] **11.3** Use `logo.png` (logo with background) for the window title bar logo (`load_embedded_egui_icon` in `src/tray/mod.rs`) while keeping `logo_bg.png` (no background) for the taskbar / system tray icon.
- [x] **11.4** Replace missing arrow glyphs on `Move Left` and `Move Right` in the Customize Button dialog with crisp vector arrows (`ToolbarIcon::ArrowLeft`, `ToolbarIcon::ArrowRight`), and enforce a hard minimum size limit (`480×620`) so no dialog elements get hidden behind the edge (`src/ui/settings_panel.rs`).
- [x] **11.5** **Verification Gate 11:** Run `cargo test`, build release binary (`cargo build --release`), commit to Git, and launch.

---

## Checkpoint Progress Log
- **Current Phase:** All Checkpoints (0 through 11) Completed and Verified.
