# Lyre

<img width="760" height="760" alt="logo" src="https://github.com/user-attachments/assets/ca2ec17a-e640-4c10-869f-f7ab33af9081" />
**Lyre** is a lightweight, ultra-fast Windows 10/11 desktop utility built in Rust for switching default audio output (`eRender`) and microphone input (`eCapture`) devices with a single click.

Designed with a **Linear / Raycast Matte Obsidian** studio aesthetic, Lyre gives you a customizable hardware-style control deck on your desktop or system tray.

---

## Key Features

- **Single-Click Audio Switching:** Instantly switches Windows default audio output and input endpoints across all roles (`eConsole`, `eMultimedia`, `eCommunications`) via Windows CoreAudio (`IPolicyConfig`), with automatic **SteelSeries Sonar** hardware redirection when Sonar is active and seamless fallback on PCs/laptops without Sonar.
- **Hover + Scroll-Wheel Volume Control:** Hover over any device button card and scroll the mouse wheel to adjust that specific device's volume in **2% increments**, complete with a brief auto-fading HUD readout (`VOL XX%`).
- **Middle-Click Per-Device Mute:** Click the scroll wheel (**Middle-Click**) on any button card to mute or unmute that single device, or use the top toolbar (`Mute Out` / `Mute Mic`) to mute all outputs or microphones at once.
- **Studio Hardware LED & Live VU Meter:**
  - Rectangular studio LED indicator on every card (**Emerald Green** when active, **Crimson Red** when muted, recessed **Matte Grey** when idle).
  - Toggleable, silky-smooth animated **Studio VU Meter** on the background of button cards driven by real-time `IAudioMeterInformation` peak levels.
- **Full Button Customization:**
  - 7 built-in scalable SVG presets (*Speakers, Headphones, IEMs, Studio Monitors, Microphone, Headset Mic, DAC/Amp*) plus support for custom transparent `.png`, `.webp`, `.svg`, `.ico`, and `.jpg` images.
  - Custom accent colors, number badges, optional text labels (`Bitcount Prop Single`), and drag-and-drop grid reordering.
- **Minimalist Frameless Mode & System Tray:**
  - Press `F2` (or `Ctrl + ,`) to toggle between **Configuration Mode** and a frameless **Minimalist Buttons-Only Mode**.
  - Pin **Always on Top**, minimize/close to the Windows System Tray, and optionally **Launch at Windows Startup**.

---

## Building from Source

### Prerequisites
- Windows 10 or Windows 11
- [Rust toolchain](https://rustup.rs/) (`stable-x86_64-pc-windows-msvc`)

### Build & Run
```powershell
cargo test
cargo build --release
.\target\release\lyre.exe
```
