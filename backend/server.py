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
import random
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
_calendar_cache = {}  # {cal_id: {"data": None, "timestamp": 0.0}}
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
            "id": "morning",
            "name": "Standard",
            "start_time": "00:00",
            "end_time": "23:59",
            "greetings": ["Guten Tag!", "Hallo zusammen!", "Moin!"],
            "sleep_mode": {
                "enabled": True,
                "display_update_interval_seconds": 60,
                "wifi_update_interval_seconds": 1800,
                "sleep_until_next_period": False
            }
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


def validate_config(config: dict) -> Optional[str]:
    """Validate that schedule sleep_mode combinations are mutually consistent."""
    schedules = config.get("schedules")
    if not isinstance(schedules, list):
        return "Missing or invalid 'schedules' list"

    for idx, s in enumerate(schedules):
        label = s.get("name") or s.get("id") or f"#{idx + 1}"
        sm = s.get("sleep_mode", {})
        if not isinstance(sm, dict):
            return f"Schedule '{label}': 'sleep_mode' must be an object"

        enabled = sm.get("enabled", True)
        sleep_until_next = sm.get("sleep_until_next_period", False)
        wifi_interval = sm.get("wifi_update_interval_seconds")
        legacy_interval = sm.get("update_interval_seconds")
        display_interval = sm.get("display_update_interval_seconds")
        has_wifi = wifi_interval is not None or legacy_interval is not None
        has_display = display_interval is not None

        if not enabled:
            if sleep_until_next:
                return f"Schedule '{label}': 'sleep_until_next_period' cannot be true when sleep mode is disabled"
            if has_wifi:
                return f"Schedule '{label}': Wi-Fi update interval cannot be set when sleep mode is disabled"
            if has_display:
                return f"Schedule '{label}': Display update interval cannot be set when sleep mode is disabled"
        elif sleep_until_next:
            if has_wifi:
                return f"Schedule '{label}': Wi-Fi update interval cannot be set when 'sleep_until_next_period' is enabled"
            if has_display:
                return f"Schedule '{label}': Display update interval cannot be set when 'sleep_until_next_period' is enabled"
        else:
            effective_wifi = wifi_interval if wifi_interval is not None else (1800 if legacy_interval is None else legacy_interval)
            if isinstance(effective_wifi, bool) or not isinstance(effective_wifi, (int, float)) or effective_wifi < 120 or effective_wifi > 43200:
                return f"Schedule '{label}': Wi-Fi update interval must be between 120 and 43200 seconds"
            if has_display:
                if isinstance(display_interval, bool) or not isinstance(display_interval, (int, float)) or display_interval < 60:
                    return f"Schedule '{label}': Display update interval must be at least 60 seconds"
                if display_interval >= effective_wifi:
                    return f"Schedule '{label}': Display update interval must be shorter than Wi-Fi update interval"

    return None


def calculate_next_update(config: dict, schedule: dict, now: datetime, next_event_time: datetime = None) -> dict:
    """
    Precalculate when the Kobo should wake up for Wi-Fi data fetch and optional offline clock ticks.
    Considers:
    1. Sleep mode enabled/disabled
    2. Sleep until next period setting (e.g. at night)
    3. Configured Wi-Fi fetch interval, schedule end time, and upcoming calendar event
    4. Optional display-only (clock) wakeup interval with Wi-Fi kept off
    """
    sleep_cfg = schedule.get("sleep_mode", {})
    enabled = bool(sleep_cfg.get("enabled", True))
    sleep_until_next = bool(sleep_cfg.get("sleep_until_next_period", False)) if enabled else False
    raw_wifi = sleep_cfg.get("wifi_update_interval_seconds")
    if raw_wifi is None:
        raw_wifi = sleep_cfg.get("update_interval_seconds")
    default_interval = int(raw_wifi or 1800)

    raw_display = sleep_cfg.get("display_update_interval_seconds")
    display_interval = int(raw_display) if (enabled and not sleep_until_next and raw_display is not None) else None

    end_time_str = schedule.get("end_time", "23:59")
    period_end = get_next_occurrence(end_time_str, now)
    seconds_to_period_end = max(120, int((period_end - now).total_seconds()))

    if not enabled:
        wake_seconds = min(1800, seconds_to_period_end)
    elif sleep_until_next:
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
    if display_interval is not None and (display_interval < 60 or display_interval >= wake_seconds):
        display_interval = None

    wake_dt = now + timedelta(seconds=wake_seconds)
    time_fmt = config.get("theme", {}).get("time_format", "24h")
    if time_fmt == "12h":
        formatted_wake = wake_dt.strftime("%I:%M %p").lstrip("0")
    else:
        formatted_wake = wake_dt.strftime("%H:%M")

    return {
        "enabled": enabled,
        "sleep_wake_secs": wake_seconds,
        "display_update_interval_secs": display_interval,
        "next_update_at": wake_dt.isoformat(),
        "sleep_note": f"Nächstes Update um {formatted_wake}" if enabled else None,
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
        if not _weather_cache.get("data"):
            _weather_cache = {"data": weather_client.get_mock_weather(unit), "timestamp": now_ts}
        return _weather_cache["data"]


def get_configured_calendars(config: dict) -> list[dict]:
    """Retrieve the list of calendar configurations, supporting both 'calendars' and legacy 'calendar'."""
    if "calendars" in config and isinstance(config["calendars"], list) and len(config["calendars"]) > 0:
        return config["calendars"]
    
    # Fallback to legacy single calendar
    legacy = config.get("calendar", {})
    return [
        {
            "id": "person1",
            "name": "Person 1",
            "enabled": legacy.get("enabled", True),
            "ical_urls": legacy.get("ical_urls", []),
            "fallback_events": legacy.get("fallback_events", [])
        },
        {
            "id": "person2",
            "name": "Person 2",
            "enabled": True,
            "ical_urls": [],
            "fallback_events": []
        }
    ]


def get_person_calendar_events(cal_cfg: dict, now: datetime) -> tuple[list, Optional[datetime]]:
    """Fetch, parse, and filter events for a specific person's calendar, always returning next 4 events."""
    global _calendar_cache
    if not cal_cfg.get("enabled", True):
        return [], None

    cal_id = cal_cfg.get("id") or cal_cfg.get("name", "person")
    ttl = cal_cfg.get("cache_ttl_seconds", 900)
    now_ts = time.time()
    raw_events = []

    cached = _calendar_cache.get(cal_id)
    if cached and (now_ts - cached["timestamp"] < ttl):
        raw_events = cached["data"]
    else:
        urls = []
        if "ical_url" in cal_cfg and cal_cfg["ical_url"]:
            urls.append(cal_cfg["ical_url"])
        if "ical_urls" in cal_cfg and isinstance(cal_cfg["ical_urls"], list):
            urls.extend(cal_cfg["ical_urls"])

        tz = now.tzinfo or timezone.utc

        for url in urls:
            if not url or not url.strip():
                continue
            try:
                content = calendar_parser.fetch_ical_url(url.strip())
                parsed = calendar_parser.parse_ics_events(content, tz)
                raw_events.extend(parsed)
            except Exception as e:
                sys.stderr.write(f"Error fetching calendar {url} for {cal_id}: {e}\n")

        # If no events found or no URLs configured, use fallback events
        if not raw_events:
            fallbacks = cal_cfg.get("fallback_events", [])
            for item in fallbacks:
                is_tomorrow = (item.get("date_label") in ("Tomorrow", "Morgen"))
                ev_date = (now + timedelta(days=1)).date() if is_tomorrow else now.date()
                raw_events.append({
                    "title": item.get("title", "Termin"),
                    "start": datetime.combine(ev_date, dtime(9, 0), tz),
                    "end": datetime.combine(ev_date, dtime(10, 0), tz),
                    "location": item.get("location", ""),
                    "description": "",
                    "is_all_day": item.get("is_all_day", False),
                })

        _calendar_cache[cal_id] = {"data": raw_events, "timestamp": now_ts}

    # Always return next 4 events
    formatted = calendar_parser.filter_and_format_events(raw_events, scope="all_upcoming", reference_dt=now, max_events=4)

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
    Horizontal-only dashboard layout:
    - Row 1: Greeting + Weather (current & forecast)
    - Row 2: Calendars for two persons side by side in separate columns (4 events each)
    - Bottom: Deep sleep instructions & wake interval
    """
    now = get_current_time(config)

    if schedule_id_override:
        schedule = next((s for s in config.get("schedules", []) if s.get("id") == schedule_id_override), None)
        if not schedule:
            schedule = find_active_schedule(config, now)
    else:
        schedule = find_active_schedule(config, now)

    # 1. Weather Data (Current + Forecast)
    weather_data = get_weather_data(config)

    # 2. Calendars for both persons (next 4 events each)
    cal_configs = get_configured_calendars(config)
    calendars_payload = []
    earliest_event_dt = None

    for cal_cfg in cal_configs[:2]:  # support two persons
        person_name = cal_cfg.get("name", "Person")
        events, next_dt = get_person_calendar_events(cal_cfg, now)
        calendars_payload.append({
            "id": cal_cfg.get("id", person_name.lower().replace(" ", "_")),
            "person": person_name,
            "events": events
        })
        if next_dt:
            if earliest_event_dt is None or next_dt < earliest_event_dt:
                earliest_event_dt = next_dt

    # If fewer than 2 persons configured, ensure a second person exists
    if len(calendars_payload) < 2:
        default_person_name = "Person 2"
        calendars_payload.append({
            "id": "person2",
            "person": default_person_name,
            "events": []
        })

    # 3. Calculate Sleep / Next update
    sleep_info = calculate_next_update(config, schedule, now, earliest_event_dt)

    # 4. Format Greeting (Person-independent, random choice from options per schedule)
    greetings_list = schedule.get("greetings")
    if isinstance(greetings_list, list) and len(greetings_list) > 0:
        title = random.choice(greetings_list)
    elif isinstance(schedule.get("greeting"), dict):
        title = schedule.get("greeting", {}).get("title", "Guten Tag!")
    else:
        title = "Guten Tag!"

    # Clean any legacy {name} placeholders
    title = title.replace(", {name}", "").replace(" {name}", "").replace("{name}", "")

    # Subtitle: German day and date
    german_weekdays = ["Montag", "Dienstag", "Mittwoch", "Donnerstag", "Freitag", "Samstag", "Sonntag"]
    german_months = ["Januar", "Februar", "März", "April", "Mai", "Juni", "Juli", "August", "September", "Oktober", "November", "Dezember"]
    wd = german_weekdays[now.weekday()]
    mo = german_months[now.month - 1]
    subtitle = f"{wd}, {now.day}. {mo}"

    sched_id = schedule.get("id", "")
    icon = "moon" if sched_id in ("evening", "night") else "sun"

    greeting_data = {
        "title": title,
        "subtitle": subtitle,
        "icon": icon
    }

    # 5. Fixed Horizontal Cards Layout:
    # Card 1: Greeting
    # Card 2: Weather (Current + 4-day forecast)
    # Card 3: Calendar Person 1 (Next 4 events)
    # Card 4: Calendar Person 2 (Next 4 events)
    person1_cal = calendars_payload[0]
    person2_cal = calendars_payload[1]

    resolved_cards = [
        {
            "type": "greeting",
            "title": greeting_data["title"],
            "temperature": weather_data.get("temperature"),
            "condition": weather_data.get("condition"),
            "humidity": weather_data.get("humidity")
        },
        {
            "type": "weather",
            "title": "Wetter",
            "forecast_days": 4,
            "data": weather_data
        },
        {
            "type": "calendar",
            "title": person1_cal["person"],
            "events": person1_cal["events"]
        },
        {
            "type": "calendar",
            "title": person2_cal["person"],
            "events": person2_cal["events"]
        }
    ]

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
            "mode": "high_contrast_bw",
            "orientation": "landscape",
            "card_corner_radius": theme_cfg.get("card_corner_radius", 18.0),
            "time_format": theme_cfg.get("time_format", "24h"),
            "language": theme_cfg.get("language", "de")
        },
        "greeting": greeting_data,
        "weather": weather_data,
        "calendars": calendars_payload,
        "calendar": {
            "scope": "all_upcoming",
            "events": person1_cal["events"] + person2_cal["events"]
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
                
                # Validate config structure and mutually exclusive sleep_mode rules
                err = validate_config(new_config)
                if err:
                    self.send_json({"status": "error", "message": err}, status=400)
                    return

                save_config(new_config)
                # Clear caches so new coordinates or URLs take effect immediately
                global _weather_cache, _calendar_cache
                _weather_cache = {"data": None, "timestamp": 0.0}
                _calendar_cache = {}

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
