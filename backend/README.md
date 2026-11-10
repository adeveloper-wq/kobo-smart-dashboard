# Kobo Dashboard - Python Backend & Web Builder

A lean, zero-dependency backend and browser-based configuration builder for the Kobo e-reader dashboard.
Runs on **Ubuntu / Debian / Linux / macOS / Windows** using **only the Python standard library** (no `pip install` required).

---

## Features

- **Zero Dependencies**: Uses Python's built-in `http.server`, `urllib.request`, `json`, and `datetime`.
- **Dynamic Time-of-Day Schedules**:
  - **Morning (06:00 - 12:00)**: Warm morning greeting, today's schedule, current weather & humidity.
  - **Afternoon (12:00 - 18:00)**: Afternoon greeting, remaining tasks for today, weather.
  - **Evening (18:00 - 22:30)**: Evening greeting, **tomorrow's agenda at a glance**, weather.
  - **Night (22:30 - 06:00)**: Night greeting, minimal overview, and deep sleep mode.
- **Google Calendar / Gmail Integration**:
  - Connects to Google Calendar via your private/secret iCal (`.ics`) feed URL.
  - Automatically filters events based on the active schedule (`today`, `today_remaining`, `tomorrow`, `next_24h`).
- **Free Weather & Humidity (Open-Meteo)**:
  - Fetches local temperature, weather condition, forecast, and relative humidity percentage.
  - No API keys or registration required.
- **Smart E-Ink Sleep & Wake Precalculation**:
  - The backend calculates exactly when the next refresh is needed (e.g. at period change, next event, or custom interval).
  - Tells the Kobo to shut off Wi-Fi, program its hardware RTC alarm, and wake up only when fresh data is needed.
- **Browser-Based Config Builder**:
  - Visual single-page web app at `http://<server-ip>:8000/`.
  - Edit schedules, time ranges, greetings, calendar feeds, weather coordinates, and themes.
  - Live pixel-accurate Kobo screen mockup preview.
  - Full bidirectional raw JSON editor with syntax validation.

---

## Quick Start on Ubuntu

### Mode 1: Lean 24/7 Server (No Builder) — Recommended for Background Service
To run an ultra-lightweight API daemon that serves only the Kobo reader without serving the web editor UI:

```bash
cd backend
python3 server.py --no-builder
```
Or in `config.json`, set `"enable_builder": false` under `"server"`.
In this mode:
- Static HTML/JS files are never served.
- `/api/config` modification is locked/disabled.
- Runs with absolute minimum memory and CPU footprint.

### Mode 2: Run the Config Builder on Demand
Whenever you want to design, modify, or preview your dashboard schedules:

```bash
python3 builder.py
```
This starts the builder tool on `http://localhost:8001/` and automatically opens your browser. When you are done editing, click **Quit** in the UI or press `Ctrl+C` to stop it.

---

### Mode 3: Combined Server (API + Web Builder)
If you want the web builder available on the same port:
```bash
python3 server.py --builder
```
Open `http://localhost:8000/` in your browser.

---

## Connecting Google Calendar / Gmail

1. Open **Google Calendar** on your computer.
2. In the left sidebar, find your calendar, click the three dots `⋮` → **Settings and sharing**.
3. Scroll down to the **Integrate calendar** section.
4. Copy the link labeled **Secret address in iCal format** (`https://calendar.google.com/calendar/ical/.../basic.ics`).
5. In the Web Builder, go to **Weather & Calendar** and paste the URL into the **Secret iCal URL** field.
6. Click **Save Config**.

---

## Running as a Background Systemd Service (Ubuntu)

To have the backend run automatically on your Ubuntu server at boot:

1. Create a service file:
```bash
sudo nano /etc/systemd/system/kobo-dashboard.service
```

2. Paste the following configuration (replace `/path/to/kobo-ha-dashboard` and `your_user`):
```ini
[Unit]
Description=Kobo E-Reader Dashboard Backend
After=network.target

[Service]
Type=simple
User=your_user
WorkingDirectory=/path/to/kobo-ha-dashboard/backend
ExecStart=/usr/bin/python3 /path/to/kobo-ha-dashboard/backend/server.py --no-builder
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
```

3. Enable and start the service:
```bash
sudo systemctl daemon-reload
sudo systemctl enable kobo-dashboard
sudo systemctl start kobo-dashboard
```

Check status:
```bash
sudo systemctl status kobo-dashboard
```

---

## Running Tests

Run the test suite:
```bash
python3 test_backend.py
```
