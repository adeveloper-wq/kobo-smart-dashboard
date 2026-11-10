# Kobo E-Reader Dashboard

A low-power, ambient e-ink dashboard for Kobo e-readers powered by a lean Python backend. Turn any Kobo reader into a smart desk or wall display showing dynamic greetings, Google Calendar / Gmail schedules, weather forecasts, and indoor humidity.

---

## Concept & Architecture

```
[ Google Calendar / Gmail ]       [ Open-Meteo Weather API ]
    (Private iCal Feed)              (No API Key Required)
            │                                  │
            └───────────────┬──────────────────┘
                            ▼
      ┌──────────────────────────────────────────────┐
      │         Lean Python Backend Server           │
      │                  (backend/)                  │
      │                                              │
      │  • Multi-schedule engine (Morning/Eve/Night) │
      │  • Dynamic greetings & event filtering       │
      │  • Precalculates sleep duration & RTC wake   │
      │  • Standalone visual Config Builder tool     │
      └─────────────────────┬────────────────────────┘
                            │  GET /api/dashboard (Local Wi-Fi)
                            ▼
              ┌───────────────────────────┐
              │    Kobo E-Reader Client   │
              │          (kobo/)          │
              │                           │
              │  • Native Rust app        │
              │  • Direct /dev/fb0 e-ink  │
              │  • Deep sleep / Wi-Fi off │
              │  • Wakes on RTC alarm     │
              └───────────────────────────┘
```

The system is split into two lightweight, decoupled components:

1. **[Backend Server (`backend/`)](backend/README.md)**:
   - Written in pure Python using **only the standard library** (runs on Ubuntu / Linux / macOS / Windows with no `pip install` dependencies).
   - Fetches your Google Calendar events and Open-Meteo weather/humidity data.
   - Dynamically adapts to the time of day: in the morning it serves today's schedule; in the evening it automatically switches to tomorrow's agenda.
   - Precalculates the exact sleep interval so the Kobo only wakes up when a refresh is necessary.
   - Includes a browser-based visual configuration builder (`builder.py`).

2. **[Kobo E-Reader Client (`kobo/`)](kobo/README.md)**:
   - A native ARMv7 binary written in Rust that runs directly on the Kobo's embedded Linux OS.
   - Draws directly into video memory (`/dev/fb0`) with hardware e-ink waveform refreshes (`hwtcon` / `mxcfb`) without running a web browser or Android layer.
   - Shuts off Wi-Fi and enters deep sleep (suspend to RAM) between updates, consuming virtually zero battery while displaying static e-ink data.
   - Launched via NickelMenu; seamlessly pauses and resumes the Kobo reading app.

---

## Directory Overview

- **[`backend/`](backend/)**: The Python server and web builder tool.
  - [Backend Setup & Usage Guide](backend/README.md)
  - `server.py`: Lean 24/7 API server for the Kobo.
  - `builder.py`: Standalone visual config editor and simulator.
  - `config.json`: Multi-schedule time-of-day configuration.
  - `calendar_parser.py`: Zero-dependency iCal / ICS feed parser.
  - `weather.py`: Zero-dependency Open-Meteo weather client.
- **[`kobo/`](kobo/)**: The native Rust client for Kobo devices.
  - [Kobo Client Build & Install Guide](kobo/README.md)
  - `src/`: Native Rust renderer and framebuffer driver.
  - `deploy/`: Cross-compilation helper and USB installation scripts.
  - `config.example.yaml`: Device configuration file.

---

## Quick Links

- **Setting up the Backend & Google Calendar**: see [backend/README.md](backend/README.md).
- **Building and Installing on the Kobo**: see [kobo/README.md](kobo/README.md).

---

## License

[MIT](LICENSE)
