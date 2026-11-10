"""
Comprehensive test suite for the Python backend using only standard library unittest.
"""

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
            }
        ]

        # Scope: today
        today_ev = calendar_parser.filter_and_format_events(events, scope="today", reference_dt=ref_dt)
        self.assertEqual(len(today_ev), 2)

        # Scope: today_remaining (ref_dt is 10:00, Morning Meeting ended at 09:30)
        rem_ev = calendar_parser.filter_and_format_events(events, scope="today_remaining", reference_dt=ref_dt)
        self.assertEqual(len(rem_ev), 1)
        self.assertEqual(rem_ev[0]["title"], "Afternoon Call")

        # Scope: tomorrow
        tom_ev = calendar_parser.filter_and_format_events(events, scope="tomorrow", reference_dt=ref_dt)
        self.assertEqual(len(tom_ev), 1)
        self.assertEqual(tom_ev[0]["title"], "Tomorrow Dentist")
        self.assertEqual(tom_ev[0]["date_label"], "Morgen")


class TestServerLogic(unittest.TestCase):
    def setUp(self):
        self.config = server.load_config()

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

    def test_next_update_precalculation(self):
        tz = timezone.utc
        now = datetime(2026, 11, 10, 10, 0, tzinfo=tz)
        sched = {"end_time": "12:00", "sleep_mode": {"enabled": True, "sleep_after_secs": 30, "update_interval_seconds": 1800}}
        update = server.calculate_next_update(self.config, sched, now)
        self.assertIn("sleep_wake_secs", update)
        self.assertGreaterEqual(update["sleep_wake_secs"], 120)
        self.assertEqual(update["sleep_after_secs"], 30)
        self.assertTrue(update["enabled"])
        self.assertIn("sleep_note", update)

    def test_dashboard_payload_structure(self):
        payload = server.build_dashboard_payload(self.config)
        self.assertEqual(payload["status"], "ok")
        self.assertIn("greeting", payload)
        self.assertIn("weather", payload)
        self.assertIn("calendar", payload)
        self.assertIn("cards", payload)
        self.assertIn("sleep", payload)
        self.assertIn("sleep_wake_secs", payload["sleep"])

    def test_builder_toggle(self):
        server.ENABLE_BUILDER = False
        self.assertFalse(server.ENABLE_BUILDER)
        server.ENABLE_BUILDER = True
        self.assertTrue(server.ENABLE_BUILDER)


if __name__ == "__main__":
    unittest.main()
