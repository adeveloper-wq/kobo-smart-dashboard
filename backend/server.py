#!/usr/bin/env python3
"""
Kobo E-Reader Dashboard Backend
================================
A lean, self-contained Python web server using ONLY standard library modules.
Runs on Ubuntu / Linux / macOS / Windows with zero external pip dependencies.

Features:
- Consumes config.json defining time-of-day schedules (morning, afternoon, evening, night).
- Dynamically assembles dashboard JSON: greetings, Open-Meteo weather & humidity, Google Calendar events.
- Precalculates exact sleep duration & next update timestamp for Kobo e-ink power saving.
- Provides browser-based config JSON builder UI at http://localhost:8000.
- Serves /api/dashboard endpoint for the Kobo reader.
"""

import os
import sys
import json
import time
import re
import urllib.parse
from pathlib import Path
from http.server import ThreadingHTTPServer, BaseHTTPRequestHandler
from typing import Any, Dict, List, Optional
from datetime import datetime, date, time as dtime, timedelta, timezone

# Add backend directory to sys.path so sibling imports work
BACKEND_DIR = Path(__file__).resolve().parent
if str(BACKEND_DIR) not in sys.path:
    sys.path.insert(0, str(BACKEND_DIR))

import calendar_parser
import weather as weather_client


# Try importing ZoneInfo (available in Python 3.9+ standard library)
try:
    from zoneinfo import ZoneInfo
except ImportError:
    ZoneInfo = None


DEFAULT_CONFIG_PATH = BACKEND_DIR / "config.json"
STATIC_DIR = BACKEND_DIR / "static"


# In-memory caches with timestamps
_weather_cache = {"data": None, "timestamp": 0.0}
_calendar_cache = {"data": None, "timestamp": 0.0}
ENABLE_BUILDER = True


def get_timezone(tz_name: str) -> timezone:
    """Resolve timezone object from string name, with safe fallbacks."""
    if ZoneInfo and tz_name:
        try:
            return ZoneInfo(tz_name)
        except Exception:
            pass
    # Fallback to local or UTC
    return datetime.now().astimezone().tzinfo or timezone.utc


def load_config(config_path: Path = DEFAULT_CONFIG_PATH) -> dict:
    """Load configuration from JSON file (falls back to config.example.json)."""
    if not config_path.exists():
        example_path = config_path.parent / "config.example.json"
        if example_path.exists():
            import shutil
            try:
                shutil.copy(example_path, config_path)
            except Exception:
                pass
        if not config_path.exists() and not example_path.exists():
            raise FileNotFoundError(f"Config file not found: {config_path}")
        path_to_read = config_path if config_path.exists() else example_path
    else:
        path_to_read = config_path

    with open(path_to_read, "r", encoding="utf-8") as f:
        return json.load(f)


def save_config(new_config: dict, config_path: Path = DEFAULT_CONFIG_PATH) -> None:
    """Save configuration to JSON file safely with atomic replace."""
    tmp_path = config_path.with_suffix(".tmp")
    with open(tmp_path, "w", encoding="utf-8") as f:
        json.dump(new_config, f, indent=2, ensure_ascii=False)
    tmp_path.replace(config_path)


def get_current_time(config: dict) -> datetime:
    """Get current datetime in configured timezone."""
    tz_name = config.get("server", {}).get("timezone", "")
    tz = get_timezone(tz_name)
    return datetime.now(tz)


def is_time_in_range(start_str: str, end_str: str, current_time: dtime) -> bool:
    """Check if current time is within HH:MM - HH:MM, handling overnight spans."""
    sh, sm = map(int, start_str.split(":"))
    eh, em = map(int, end_str.split(":"))
    start = dtime(sh, sm)
    end = dtime(eh, em)

    if start <= end:
        return start <= current_time < end
    else:
        # Crosses midnight, e.g. 22:30 to 06:00
        return current_time >= start or current_time < end


def find_active_schedule(config: dict, now: datetime) -> dict:
    """Find the schedule matching current time, or default to the first one."""
    schedules = config.get("schedules", [])
    if not schedules:
        return {
            "id": "default",
            "name": "Default Dashboard",
            "start_time": "00:00",
            "end_time": "23:59",
            "greeting": {"title": "Hello, {name}!", "subtitle": "Welcome to your dashboard", "icon": "sun"},
            "calendar_scope": "today",
            "cards": [{"type": "greeting"}, {"type": "weather"}, {"type": "calendar"}, {"type": "humidity"}],
            "sleep_mode": {"enabled": True, "update_interval_seconds": 1800}
        }

    cur_time = now.time()
    for s in schedules:
        start_str = s.get("start_time", "00:00")
        end_str = s.get("end_time", "23:59")
        if is_time_in_range(start_str, end_str, cur_time):
            return s

    return schedules[0]


def get_next_occurrence(time_str: str, now: datetime) -> datetime:
    """Return the next datetime when HH:MM occurs."""
    h, m = map(int, time_str.split(":"))
    target = now.replace(hour=h, minute=m, second=0, microsecond=0)
    if target <= now:
        target += timedelta(days=1)
    return target


def calculate_next_update(config: dict, schedule: dict, now: datetime, next_event_time: datetime = None) -> dict:
    """
    Precalculate when the Kobo should wake up and turn Wi-Fi on again.
    Considers:
    1. Schedule end time (period transition)
    2. Upcoming calendar event
    3. Configured update interval
    4. Sleep until next period setting (e.g. at night)
    """
    sleep_cfg = schedule.get("sleep_mode", {})
    enabled = sleep_cfg.get("enabled", True)
    default_interval = sleep_cfg.get("update_interval_seconds", 1800)
    sleep_until_next = sleep_cfg.get("sleep_until_next_period", False)

    end_time_str = schedule.get("end_time", "23:59")
    period_end = get_next_occurrence(end_time_str, now)
    seconds_to_period_end = max(120, int((period_end - now).total_seconds()))

    if sleep_until_next:
        # Deep sleep until the next schedule begins
        wake_seconds = seconds_to_period_end
    else:
        wake_seconds = default_interval
        # If period ends sooner than interval, wake up for period change
        if seconds_to_period_end < wake_seconds:
            wake_seconds = seconds_to_period_end
        # If an event starts shortly, align update
        if next_event_time and next_event_time > now:
            secs_to_event = int((next_event_time - now).total_seconds())
            if 120 <= secs_to_event < wake_seconds:
                wake_seconds = secs_to_event

    # Clamp to reasonable limits (at least 2 minutes, at most 12 hours)
    wake_seconds = max(120, min(wake_seconds, 43200))

    wake_dt = now + timedelta(seconds=wake_seconds)
    time_fmt = config.get("theme", {}).get("time_format", "24h")
    if time_fmt == "12h":
        formatted_wake = wake_dt.strftime("%I:%M %p").lstrip("0")
    else:
        formatted_wake = wake_dt.strftime("%H:%M")

    return {
        "enabled": enabled,
        "sleep_wake_secs": wake_seconds,
        "next_update_at": wake_dt.isoformat(),
        "sleep_note": f"Next update at {formatted_wake}",
        "wake_time_formatted": formatted_wake
    }


def get_weather_data(config: dict) -> dict:
    """Fetch or return cached weather and humidity data."""
    global _weather_cache
    w_cfg = config.get("weather", {})
    if not w_cfg.get("enabled", True):
        return weather_client.get_mock_weather()

    ttl = w_cfg.get("cache_ttl_seconds", 900)
    now_ts = time.time()
    if _weather_cache["data"] and (now_ts - _weather_cache["timestamp"] < ttl):
        return _weather_cache["data"]

    lat = float(w_cfg.get("latitude", 52.52))
    lon = float(w_cfg.get("longitude", 13.405))
    unit = w_cfg.get("temperature_unit", "celsius")

    try:
        data = weather_client.fetch_open_meteo_weather(lat, lon, unit)
        _weather_cache = {"data": data, "timestamp": now_ts}
        return data
    except Exception as e:
        sys.stderr.write(f"Weather fetch failed: {e}. Using cached/fallback data.\n")
        if _weather_cache["data"]:
            return _weather_cache["data"]
        return weather_client.get_mock_weather(unit)


def get_calendar_events(config: dict, scope: str, now: datetime) -> tuple[list, datetime]:
    """Fetch, parse, and filter calendar events for given scope."""
    global _calendar_cache
    cal_cfg = config.get("calendar", {})
    if not cal_cfg.get("enabled", True):
        return [], None

    ttl = cal_cfg.get("cache_ttl_seconds", 900)
    now_ts = time.time()
    raw_events = []

    if _calendar_cache["data"] and (now_ts - _calendar_cache["timestamp"] < ttl):
        raw_events = _calendar_cache["data"]
    else:
        urls = cal_cfg.get("ical_urls", [])
        tz = now.tzinfo or timezone.utc

        for url in urls:
            if not url or not url.strip():
                continue
            try:
                content = calendar_parser.fetch_ical_url(url.strip())
                parsed = calendar_parser.parse_ics_events(content, tz)
                raw_events.extend(parsed)
            except Exception as e:
                sys.stderr.write(f"Error fetching calendar {url}: {e}\n")

        # If no events found or no URLs configured, use fallback/sample events
        if not raw_events:
            fallbacks = cal_cfg.get("fallback_events", [])
            for item in fallbacks:
                is_tomorrow = (item.get("date_label") == "Tomorrow")
                ev_date = (now + timedelta(days=1)).date() if is_tomorrow else now.date()
                raw_events.append({
                    "title": item.get("title", "Event"),
                    "start": datetime.combine(ev_date, dtime(9, 0), tz),
                    "end": datetime.combine(ev_date, dtime(10, 0), tz),
                    "location": item.get("location", ""),
                    "description": "",
                    "is_all_day": item.get("is_all_day", False),
                })

        _calendar_cache = {"data": raw_events, "timestamp": now_ts}

    formatted = calendar_parser.filter_and_format_events(raw_events, scope=scope, reference_dt=now, max_events=6)

    # Find earliest upcoming event start
    next_event_dt = None
    for ev in raw_events:
        st = ev.get("start")
        if st and st > now:
            if next_event_dt is None or st < next_event_dt:
                next_event_dt = st

    return formatted, next_event_dt


def build_dashboard_payload(config: dict, schedule_id_override: str = None) -> dict:
    """
    Build the full JSON payload requested by the Kobo frontend.
    Contains theme, greetings, weather, calendar, layout cards, and sleep instructions.
    """
    now = get_current_time(config)

    if schedule_id_override:
        schedule = next((s for s in config.get("schedules", []) if s.get("id") == schedule_id_override), None)
        if not schedule:
            schedule = find_active_schedule(config, now)
    else:
        schedule = find_active_schedule(config, now)

    # 1. Fetch Weather & Humidity
    weather_data = get_weather_data(config)

    # 2. Fetch Calendar
    scope = schedule.get("calendar_scope", "today")
    calendar_events, next_event_dt = get_calendar_events(config, scope, now)

    # 3. Calculate Sleep / Next update
    sleep_info = calculate_next_update(config, schedule, now, next_event_dt)

    # 4. Format Greeting
    user_name = config.get("user", {}).get("name", "User")
    raw_greeting = schedule.get("greeting", {})
    title_tpl = raw_greeting.get("title", "Hello, {name}!")
    title = title_tpl.replace("{name}", user_name)

    subtitle = raw_greeting.get("subtitle", "")
    if "{date}" in subtitle or not subtitle:
        date_str = now.strftime("%A, %B %d")
        subtitle = f"{date_str} · {len(calendar_events)} events scheduled"

    greeting_data = {
        "title": title,
        "subtitle": subtitle,
        "icon": raw_greeting.get("icon", "sun")
    }

    # 5. Build dynamic Cards array based on schedule configuration
    resolved_cards = []
    card_configs = schedule.get("cards", [])
    
    for c in card_configs:
        c_type = c.get("type", "")
        if c_type == "greeting":
            resolved_cards.append({
                "type": "greeting",
                "title": greeting_data["title"],
                "subtitle": greeting_data["subtitle"],
                "icon": greeting_data["icon"]
            })
        elif c_type == "weather":
            resolved_cards.append({
                "type": "weather",
                "title": c.get("title", "Weather"),
                "forecast_days": c.get("forecast_days", 3),
                "data": weather_data
            })
        elif c_type == "calendar":
            calendar_title = c.get("title", "Calendar")
            resolved_cards.append({
                "type": "calendar",
                "title": calendar_title,
                "scope": scope,
                "events": calendar_events
            })
        elif c_type == "humidity":
            resolved_cards.append({
                "type": "humidity",
                "title": c.get("title", "Humidity"),
                "value": f"{weather_data.get('humidity', 50):.0f}",
                "unit": "%",
                "status": weather_data.get("humidity_status", "Comfortable"),
                "icon": "droplet"
            })
        else:
            resolved_cards.append(c)

    theme_cfg = config.get("theme", {})

    return {
        "status": "ok",
        "generated_at": now.isoformat(),
        "active_schedule": {
            "id": schedule.get("id"),
            "name": schedule.get("name"),
            "start_time": schedule.get("start_time"),
            "end_time": schedule.get("end_time")
        },
        "theme": {
            "mode": theme_cfg.get("mode", "kaleido"),
            "accent_color": theme_cfg.get("accent_color", "#D97706"),
            "card_corner_radius": theme_cfg.get("card_corner_radius", 18.0),
            "time_format": theme_cfg.get("time_format", "24h"),
            "language": theme_cfg.get("language", "en")
        },
        "greeting": greeting_data,
        "weather": weather_data,
        "calendar": {
            "scope": scope,
            "events": calendar_events
        },
        "cards": resolved_cards,
        "sleep": sleep_info
    }


class DashboardRequestHandler(BaseHTTPRequestHandler):
    """HTTP Request Handler serving API and static web editor."""

    def log_message(self, format, *args):
        # Quiet standard HTTP logging, keep console clean
        sys.stderr.write(f"[{self.log_date_time_string()}] {format % args}\n")

    def send_json(self, data: Any, status: int = 200):
        body = json.dumps(data, indent=2, ensure_ascii=False).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Access-Control-Allow-Origin", "*")
        self.end_headers()
        self.wfile.write(body)

    def do_OPTIONS(self):
        """Handle CORS pre-flight."""
        self.send_response(200)
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
        self.send_header("Access-Control-Allow-Headers", "Content-Type")
        self.end_headers()

    def do_GET(self):
        url = urllib.parse.urlparse(self.path)
        path = url.path
        query = urllib.parse.parse_qs(url.query)

        # 1. Kobo Dashboard API Endpoint
        if path == "/api/dashboard":
            try:
                config = load_config()
                payload = build_dashboard_payload(config)
                self.send_json(payload)
            except Exception as e:
                self.send_json({"status": "error", "message": str(e)}, status=500)
            return

        # 2. Preview Endpoint (can specify ?schedule=<id>)
        if path == "/api/preview":
            try:
                config = load_config()
                schedule_id = query.get("schedule", [None])[0]
                payload = build_dashboard_payload(config, schedule_id_override=schedule_id)
                self.send_json(payload)
            except Exception as e:
                self.send_json({"status": "error", "message": str(e)}, status=500)
            return

        # 3. Config JSON API Endpoint
        if path == "/api/config":
            try:
                config = load_config()
                self.send_json(config)
            except Exception as e:
                self.send_json({"status": "error", "message": str(e)}, status=500)
            return

        # 4. Status / Health check
        if path == "/api/status":
            try:
                config = load_config()
                now = get_current_time(config)
                sched = find_active_schedule(config, now)
                self.send_json({
                    "status": "healthy",
                    "server_time": now.isoformat(),
                    "active_schedule": sched.get("id"),
                    "timezone": str(now.tzinfo),
                    "builder_enabled": ENABLE_BUILDER
                })
            except Exception as e:
                self.send_json({"status": "error", "message": str(e)}, status=500)
            return

        # 5. Serve Browser-Based Config JSON Builder UI (only if enabled)
        if path in ("/", "/index.html", "/editor", "/editor.html"):
            if not ENABLE_BUILDER:
                self.send_json({
                    "status": "ok",
                    "service": "kobo-dashboard-api",
                    "mode": "api-only",
                    "message": "Config builder is disabled on this server. Run without --no-builder or use builder.py to edit config."
                })
                return

            editor_path = STATIC_DIR / "editor.html"
            if editor_path.exists():
                with open(editor_path, "rb") as f:
                    content = f.read()
                self.send_response(200)
                self.send_header("Content-Type", "text/html; charset=utf-8")
                self.send_header("Content-Length", str(len(content)))
                self.end_headers()
                self.wfile.write(content)
                return

        self.send_error(404, "Endpoint not found")

    def do_POST(self):
        url = urllib.parse.urlparse(self.path)
        path = url.path

        # Save Config JSON Endpoint (requires builder to be enabled)
        if path == "/api/config":
            if not ENABLE_BUILDER:
                self.send_json({"status": "error", "message": "Config saving is disabled in API-only mode"}, status=403)
                return

            try:
                content_len = int(self.headers.get("Content-Length", 0))
                raw_body = self.rfile.read(content_len).decode("utf-8")
                new_config = json.loads(raw_body)
                
                # Basic validation
                if "schedules" not in new_config:
                    self.send_json({"status": "error", "message": "Missing 'schedules' key"}, status=400)
                    return

                save_config(new_config)
                # Clear caches so new coordinates or URLs take effect immediately
                global _weather_cache, _calendar_cache
                _weather_cache = {"data": None, "timestamp": 0.0}
                _calendar_cache = {"data": None, "timestamp": 0.0}

                self.send_json({"status": "ok", "message": "Configuration saved successfully"})
            except Exception as e:
                self.send_json({"status": "error", "message": str(e)}, status=500)
            return

        self.send_error(404, "Endpoint not found")


def run_self_test():
    """Verify backend logic without starting network server."""
    print("Running self-test...")
    cfg = load_config()
    now = get_current_time(cfg)
    print(f"Current server time: {now}")
    sched = find_active_schedule(cfg, now)
    print(f"Active schedule for right now: {sched.get('id')} ({sched.get('name')})")
    payload = build_dashboard_payload(cfg)
    print("Generated Dashboard Payload:")
    print(f"  Greeting: {payload['greeting']['title']}")
    print(f"  Subtitle: {payload['greeting']['subtitle']}")
    print(f"  Weather: {payload['weather']['temperature']}{payload['weather']['temperature_unit']}, {payload['weather']['condition_label']}")
    print(f"  Humidity: {payload['weather']['humidity']}% ({payload['weather']['humidity_status']})")
    print(f"  Calendar scope: {payload['calendar']['scope']} ({len(payload['calendar']['events'])} events)")
    print(f"  Sleep wake interval: {payload['sleep']['sleep_wake_secs']}s ({payload['sleep']['sleep_note']})")
    print("Self-test passed successfully!")


def main():
    global ENABLE_BUILDER
    import argparse
    parser = argparse.ArgumentParser(description="Kobo Dashboard Python Backend")
    parser.add_argument("--port", type=int, default=None, help="Server port (overrides config.json)")
    parser.add_argument("--host", type=str, default=None, help="Server host (overrides config.json)")
    parser.add_argument("--test", action="store_true", help="Run self-test and exit")
    parser.add_argument("--no-builder", "--api-only", dest="no_builder", action="store_true", help="Run in lean API-only mode (builder UI disabled)")
    parser.add_argument("--builder", dest="force_builder", action="store_true", help="Explicitly enable the config builder UI")
    args = parser.parse_args()

    if args.test:
        run_self_test()
        return

    config = load_config()
    server_cfg = config.get("server", {})
    host = args.host or server_cfg.get("host", "0.0.0.0")
    port = args.port or int(server_cfg.get("port", 8000))

    if args.no_builder:
        ENABLE_BUILDER = False
    elif args.force_builder:
        ENABLE_BUILDER = True
    else:
        ENABLE_BUILDER = server_cfg.get("enable_builder", True)

    server_address = (host, port)
    httpd = ThreadingHTTPServer(server_address, DashboardRequestHandler)

    mode_label = "Full (API + Config Builder)" if ENABLE_BUILDER else "Lean (API-Only / No Builder)"

    print("=" * 65)
    print(f"  Kobo Dashboard Python Backend running on http://{host}:{port}")
    print(f"  - Mode                  : {mode_label}")
    print(f"  - Kobo API Endpoint     : http://localhost:{port}/api/dashboard")
    if ENABLE_BUILDER:
        print(f"  - Config JSON Builder UI : http://localhost:{port}/")
    else:
        print(f"  - Config Builder UI      : Disabled (run 'python builder.py' to configure)")
    print("=" * 65)

    try:
        httpd.serve_forever()
    except KeyboardInterrupt:
        print("\nStopping server...")
        httpd.server_close()


if __name__ == "__main__":
    main()
