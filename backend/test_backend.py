"""
Comprehensive test suite for the Python backend using only standard library unittest.
"""

import json
import unittest
from datetime import datetime, date, time as dtime, timedelta, timezone
from pathlib import Path
import sys

BACKEND_DIR = Path(__file__).resolve().parent
if str(BACKEND_DIR) not in sys.path:
    sys.path.insert(0, str(BACKEND_DIR))

import server
import calendar_parser
import weather


class TestCalendarParser(unittest.TestCase):
    def test_ics_unfolding(self):
        folded = "SUMMARY:This is a long line\n that was folded\n  with whitespace"
        unfolded = calendar_parser.unfold_ics(folded)
        self.assertEqual(len(unfolded), 1)
        self.assertIn("folded", unfolded[0])

    def test_datetime_parsing(self):
        dt_utc = calendar_parser.parse_ics_datetime("20261110T143000Z")
        self.assertIsNotNone(dt_utc)
        self.assertEqual(dt_utc.year, 2026)
        self.assertEqual(dt_utc.hour, 14)
        self.assertEqual(dt_utc.minute, 30)

        dt_allday = calendar_parser.parse_ics_datetime("20261110")
        self.assertIsNotNone(dt_allday)
        self.assertEqual(dt_allday.day, 10)

    def test_filter_scopes(self):
        tz = timezone.utc
        ref_dt = datetime(2026, 11, 10, 10, 0, tzinfo=tz)

        events = [
            {
                "title": "Morning Meeting",
                "start": datetime(2026, 11, 10, 9, 0, tzinfo=tz),
                "end": datetime(2026, 11, 10, 9, 30, tzinfo=tz),
                "is_all_day": False,
            },
            {
                "title": "Afternoon Call",
                "start": datetime(2026, 11, 10, 15, 0, tzinfo=tz),
                "end": datetime(2026, 11, 10, 16, 0, tzinfo=tz),
                "is_all_day": False,
            },
            {
                "title": "Tomorrow Dentist",
                "start": datetime(2026, 11, 11, 11, 0, tzinfo=tz),
                "end": datetime(2026, 11, 11, 12, 0, tzinfo=tz),
                "is_all_day": False,
            },
            {
                "title": "Day 3 Workshop",
                "start": datetime(2026, 11, 12, 14, 0, tzinfo=tz),
                "end": datetime(2026, 11, 12, 17, 0, tzinfo=tz),
                "is_all_day": False,
            },
            {
                "title": "Day 4 Planning",
                "start": datetime(2026, 11, 13, 9, 0, tzinfo=tz),
                "end": datetime(2026, 11, 13, 10, 0, tzinfo=tz),
                "is_all_day": False,
            },
            {
                "title": "Day 5 Review",
                "start": datetime(2026, 11, 14, 15, 0, tzinfo=tz),
                "end": datetime(2026, 11, 14, 16, 0, tzinfo=tz),
                "is_all_day": False,
            }
        ]

        # Scope: all_upcoming (capped at 4 events)
        all_up = calendar_parser.filter_and_format_events(events, scope="all_upcoming", reference_dt=ref_dt, max_events=4)
        self.assertEqual(len(all_up), 4)
        self.assertEqual(all_up[0]["title"], "Afternoon Call")
        self.assertEqual(all_up[1]["title"], "Tomorrow Dentist")
        self.assertEqual(all_up[2]["title"], "Day 3 Workshop")
        self.assertEqual(all_up[3]["title"], "Day 4 Planning")


class TestServerLogic(unittest.TestCase):
    def setUp(self):
        with open(BACKEND_DIR / "config.example.json", "r", encoding="utf-8") as f:
            self.config = json.load(f)

    def test_time_range_overnight(self):
        # 22:30 to 06:00
        self.assertTrue(server.is_time_in_range("22:30", "06:00", dtime(23, 0)))
        self.assertTrue(server.is_time_in_range("22:30", "06:00", dtime(1, 30)))
        self.assertTrue(server.is_time_in_range("22:30", "06:00", dtime(5, 59)))
        self.assertFalse(server.is_time_in_range("22:30", "06:00", dtime(12, 0)))
        self.assertFalse(server.is_time_in_range("22:30", "06:00", dtime(22, 15)))

    def test_schedule_resolution(self):
        tz = timezone.utc
        morning_dt = datetime(2026, 11, 10, 8, 30, tzinfo=tz)
        s_morn = server.find_active_schedule(self.config, morning_dt)
        self.assertEqual(s_morn["id"], "morning")

        afternoon_dt = datetime(2026, 11, 10, 14, 0, tzinfo=tz)
        s_aft = server.find_active_schedule(self.config, afternoon_dt)
        self.assertEqual(s_aft["id"], "afternoon")

        evening_dt = datetime(2026, 11, 10, 19, 30, tzinfo=tz)
        s_eve = server.find_active_schedule(self.config, evening_dt)
        self.assertEqual(s_eve["id"], "evening")

        night_dt = datetime(2026, 11, 10, 23, 15, tzinfo=tz)
        s_night = server.find_active_schedule(self.config, night_dt)
        self.assertEqual(s_night["id"], "night")

    def test_random_greeting_options(self):
        # Schedule has list of greeting options and returns one of them
        morning_sched = next(s for s in self.config["schedules"] if s["id"] == "morning")
        options = morning_sched["greetings"]
        self.assertIsInstance(options, list)
        self.assertGreater(len(options), 1)

        # Test multiple payload generations select from the options
        sampled = set()
        for _ in range(20):
            payload = server.build_dashboard_payload(self.config, schedule_id_override="morning")
            title = payload["greeting"]["title"]
            self.assertIn(title, options)
            sampled.add(title)
        # Random distribution should select at least 2 distinct greetings out of 4 across 20 iterations
        self.assertGreater(len(sampled), 1)

    def test_horizontal_orientation_and_layout(self):
        payload = server.build_dashboard_payload(self.config)
        self.assertEqual(payload["theme"]["orientation"], "landscape")

    def test_two_calendars_for_two_persons(self):
        payload = server.build_dashboard_payload(self.config)
        self.assertIn("calendars", payload)
        self.assertEqual(len(payload["calendars"]), 2)
        # Each person has next 4 events
        for cal in payload["calendars"]:
            self.assertLessEqual(len(cal["events"]), 4)

        # The cards array has fixed structure: Greeting, Weather, Calendar Person 1, Calendar Person 2
        cards = payload["cards"]
        self.assertEqual(len(cards), 4)
        self.assertEqual(cards[0]["type"], "greeting")
        self.assertEqual(cards[1]["type"], "weather")
        self.assertEqual(cards[2]["type"], "calendar")
        self.assertEqual(cards[3]["type"], "calendar")
        self.assertEqual(cards[2]["title"], payload["calendars"][0]["person"])
        self.assertEqual(cards[3]["title"], payload["calendars"][1]["person"])

    def test_weather_includes_forecast(self):
        payload = server.build_dashboard_payload(self.config)
        weather = payload["weather"]
        self.assertIn("temperature", weather)
        self.assertIn("forecast", weather)
        self.assertGreater(len(weather["forecast"]), 0)

    def test_next_update_precalculation(self):
        tz = timezone.utc
        now = datetime(2026, 11, 10, 10, 0, tzinfo=tz)
        sched = {
            "end_time": "12:00",
            "sleep_mode": {
                "enabled": True,
                "display_update_interval_seconds": 60,
                "wifi_update_interval_seconds": 1800
            }
        }
        update = server.calculate_next_update(self.config, sched, now)
        self.assertIn("sleep_wake_secs", update)
        self.assertGreaterEqual(update["sleep_wake_secs"], 120)
        self.assertEqual(update["display_update_interval_secs"], 60)
        self.assertNotIn("sleep_after_secs", update)
        self.assertTrue(update["enabled"])
        self.assertIsNotNone(update["sleep_note"])

        # Disabled sleep mode hides sleep note and clears display_update_interval_secs
        disabled_sched = {"end_time": "12:00", "sleep_mode": {"enabled": False, "sleep_until_next_period": True}}
        disabled_update = server.calculate_next_update(self.config, disabled_sched, now)
        self.assertFalse(disabled_update["enabled"])
        self.assertIsNone(disabled_update["display_update_interval_secs"])
        self.assertIsNone(disabled_update["sleep_note"])

        # Sleep until next period uses remaining time until end_time and disables display-only ticks
        until_end_sched = {"end_time": "12:00", "sleep_mode": {"enabled": True, "sleep_until_next_period": True}}
        until_end_update = server.calculate_next_update(self.config, until_end_sched, now)
        self.assertTrue(until_end_update["enabled"])
        self.assertIsNone(until_end_update["display_update_interval_secs"])
        self.assertEqual(until_end_update["sleep_wake_secs"], 7200)

    def test_validate_config_rejects_conflicting_sleep_mode(self):
        self.assertIsNone(server.validate_config(self.config))

        bad_disabled_until = {
            "schedules": [{"id": "m", "sleep_mode": {"enabled": False, "sleep_until_next_period": True}}]
        }
        self.assertIsNotNone(server.validate_config(bad_disabled_until))

        bad_disabled_interval = {
            "schedules": [{"id": "m", "sleep_mode": {"enabled": False, "wifi_update_interval_seconds": 1800}}]
        }
        self.assertIsNotNone(server.validate_config(bad_disabled_interval))

        bad_disabled_display = {
            "schedules": [{"id": "m", "sleep_mode": {"enabled": False, "display_update_interval_seconds": 60}}]
        }
        self.assertIsNotNone(server.validate_config(bad_disabled_display))

        bad_until_interval = {
            "schedules": [{"id": "m", "sleep_mode": {"enabled": True, "sleep_until_next_period": True, "wifi_update_interval_seconds": 1800}}]
        }
        self.assertIsNotNone(server.validate_config(bad_until_interval))

        bad_short_interval = {
            "schedules": [{"id": "m", "sleep_mode": {"enabled": True, "sleep_until_next_period": False, "wifi_update_interval_seconds": 60}}]
        }
        self.assertIsNotNone(server.validate_config(bad_short_interval))

        bad_display_ge_wifi = {
            "schedules": [{"id": "m", "sleep_mode": {"enabled": True, "sleep_until_next_period": False, "display_update_interval_seconds": 1800, "wifi_update_interval_seconds": 1800}}]
        }
        self.assertIsNotNone(server.validate_config(bad_display_ge_wifi))

        bad_display_too_short = {
            "schedules": [{"id": "m", "sleep_mode": {"enabled": True, "sleep_until_next_period": False, "display_update_interval_seconds": 30, "wifi_update_interval_seconds": 1800}}]
        }
        self.assertIsNotNone(server.validate_config(bad_display_too_short))


if __name__ == "__main__":
    unittest.main()
