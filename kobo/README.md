# Kobo E-Reader Dashboard Client

The native client application running directly on Kobo e-readers. Written in Rust for embedded Linux, it renders the dynamic dashboard directly to the e-ink screen using hardware framebuffer ioctls.

---

## Features

- **Direct Framebuffer Rendering**: Uses `tiny-skia` and `fontdue` to compose high-contrast 300 ppi e-ink graphics directly into `/dev/fb0`. No web browser or Android layer needed.
- **Hardware E-Ink Waveforms**: Native ioctls for MediaTek (`hwtcon` on Clara Colour, Clara BW, Libra Colour) and NXP (`mxcfb` on Clara HD, Libra 2, Forma, Glo, Aura).
- **Backend-Driven**: Pulls dashboard layout, time-of-day greetings, Google Calendar events, and Open-Meteo weather/humidity from the Python backend.
- **Deep Sleep & Battery Saving**: Turns off Wi-Fi and suspends to RAM (`/sys/power/state`) between updates, waking up via hardware RTC alarm exactly when the backend schedules the next refresh.
- **Nickel Integration**: Starts from [NickelMenu](https://pgaskin.net/NickelMenu/). Pauses Kobo's reading software (`nickel`) and watchdog (`sickel`) while running, and safely resumes them when exiting or connecting USB to a computer.

---

## Supported Devices

- **MediaTek Kobos**: Kobo Clara Colour, Kobo Clara BW, Libra Colour, Elipsa 2E
- **NXP / Freescale Kobos**: Clara HD, Clara 2E, Libra 2, Libra H2O, Sage, Forma, Nia, Glo, Aura, Touch
- **Tolino (Kobo-based)**: Shine 5, Shine Color, Vision Color

---

## Building from Source

### Prerequisites
1. [Rust](https://rustup.rs/) (with `armv7-unknown-linux-musleabihf` target):
   ```bash
   rustup target add armv7-unknown-linux-musleabihf
   ```
2. [Zig](https://ziglang.org/) (used as a cross-compiler linker):
   - macOS: `brew install zig`
   - Linux: `sudo apt install zig` or download from [ziglang.org](https://ziglang.org/download/)
   - Windows: `winget install zig.zig`
3. [cargo-zigbuild](https://github.com/rust-cross/cargo-zigbuild):
   ```bash
   cargo install cargo-zigbuild
   ```

### Compile ARMv7 Binary
From inside the `kobo/` directory:
```bash
./deploy/build-kobo.sh
```
Or directly with cargo:
```bash
cargo zigbuild --target armv7-unknown-linux-musleabihf --release
```
The compiled binary will be located at:
`target/armv7-unknown-linux-musleabihf/release/kobo-ha-dashboard`

---

## Installing to Kobo E-Reader

1. Connect your Kobo to your computer via USB.
2. Tap **Connect** on the Kobo screen.
3. Run the installer script from the `kobo/` directory:
   ```bash
   ./deploy/install.sh
   ```
   This script will:
   - Copy the ARM binary and scripts (`launch.sh`, `wifi.sh`) to `.kobo-ha/` on the Kobo.
   - Copy `config.example.yaml` as `config.yaml` if not already present.
   - Install the NickelMenu menu entry (`.adds/nm/kobo-ha`).
4. Eject the Kobo and disconnect the USB cable.

---

## Configuration (`config.yaml`)

On the Kobo, configuration is stored in `.kobo-ha/config.yaml`:

```yaml
# Point to your Python backend server
backend:
  url: "http://192.168.1.100:8000"   # IP of your backend server
  enabled: true

device:
  orientation: "portrait"             # "portrait" or "landscape"
  sleep_after_secs: 60                # Deep sleep after 1 min idle
  color_panel: true                   # True for Kaleido color models
```

---

## Using the Dashboard

1. Tap the menu button at the bottom-right of the Kobo home screen.
2. Tap **Home Dashboard**.
3. The dashboard connects to your Python backend over Wi-Fi, renders the current schedule (greeting, calendar, weather), precalculates the next refresh time, and enters power-saving sleep.
4. **To Exit**: Tap the Exit icon in the top header (or plug the USB cable into a PC) to return to your books.
