"""
Lean, zero-dependency iCal / ICS calendar parser using only Python standard library.
Supports Google Calendar / Gmail secret iCal feeds, recurring rules (basic), all-day events,
and date filtering for Today, Tomorrow, and upcoming periods.
"""

import re
from datetime import datetime, date, time, timedelta, timezone
from typing import List, Dict, Any, Optional
import urllib.request


def unfold_ics(text: str) -> List[str]:
    """Unfold lines according to RFC 5545 (line ending followed by whitespace)."""
    # Normalize line endings
    text = text.replace('\r\n', '\n').replace('\r', '\n')
    # Replace newline + whitespace with nothing (unfold)
    unfolded = re.sub(r'\n[ \t]', '', text)
    return [line.strip() for line in unfolded.split('\n') if line.strip()]


def parse_ics_datetime(val: str, tz_hint: Optional[timezone] = None) -> Optional[datetime]:
    """Parse various iCal date/datetime representations into datetime object."""
    val = val.strip()
    # Check for parameters like DTSTART;VALUE=DATE:20261110
    if ':' in val:
        val = val.split(':', 1)[1].strip()

    # Case 1: Date only YYYYMMDD (All day)
    if re.match(r'^\d{8}$', val):
        y, m, d = int(val[0:4]), int(val[4:6]), int(val[6:8])
        return datetime(y, m, d, 0, 0, 0, tzinfo=tz_hint or timezone.utc)

    # Case 2: UTC Datetime YYYYMMDDTHHMMSSZ
    if val.endswith('Z') and 'T' in val:
        val_clean = val[:-1]
        try:
            dt = datetime.strptime(val_clean, '%Y%m%dT%H%M%S')
            return dt.replace(tzinfo=timezone.utc)
        except ValueError:
            pass

    # Case 3: Local/floating Datetime YYYYMMDDTHHMMSS
    if 'T' in val:
        try:
            dt = datetime.strptime(val, '%Y%m%dT%H%M%S')
            return dt.replace(tzinfo=tz_hint or timezone.utc)
        except ValueError:
            pass

    return None


def parse_ics_events(ics_content: str, local_tz: Optional[timezone] = None) -> List[Dict[str, Any]]:
    """Parse events from an iCal (.ics) string."""
    lines = unfold_ics(ics_content)
    events = []
    in_event = False
    current: Dict[str, str] = {}

    for line in lines:
        if line == 'BEGIN:VEVENT':
            in_event = True
            current = {}
        elif line == 'END:VEVENT':
            if in_event and 'DTSTART' in current:
                dtstart = parse_ics_datetime(current['DTSTART'], local_tz)
                dtend = parse_ics_datetime(current.get('DTEND', current['DTSTART']), local_tz)
                
                is_all_day = 'VALUE=DATE' in current.get('DTSTART_RAW', '') or (
                    dtstart and dtend and dtstart.hour == 0 and dtstart.minute == 0 and dtend.hour == 0 and dtend.minute == 0 and (dtend - dtstart).days >= 1
                )

                if dtstart:
                    summary = current.get('SUMMARY', 'Termin ohne Titel')
                    # Unescape commas, semicolons, backslashes
                    summary = summary.replace('\\,', ',').replace('\\;', ';').replace('\\\\', '\\').replace('\\n', ' ')
                    location = current.get('LOCATION', '').replace('\\,', ',').replace('\\;', ';')
                    desc = current.get('DESCRIPTION', '').replace('\\n', ' ')

                    events.append({
                        'title': summary,
                        'start': dtstart,
                        'end': dtend or dtstart,
                        'location': location,
                        'description': desc,
                        'is_all_day': is_all_day,
                    })
            in_event = False
        elif in_event:
            if ':' in line:
                key_part, val_part = line.split(':', 1)
                key = key_part.split(';')[0].upper()
                current[key] = val_part
                current[f'{key}_RAW'] = line

    return events


def filter_and_format_events(
    events: List[Dict[str, Any]],
    scope: str = 'all_upcoming',
    reference_dt: Optional[datetime] = None,
    max_events: int = 7
) -> List[Dict[str, Any]]:
    """
    Filter events by scope ('all_upcoming', 'today', 'today_remaining', 'tomorrow', 'next_24h')
    and format them into friendly JSON dictionaries for display.
    Defaults to all upcoming events, returning the next 4 events.
    """
    if reference_dt is None:
        reference_dt = datetime.now().astimezone()

    ref_date = reference_dt.date()
    tomorrow_date = ref_date + timedelta(days=1)

    filtered = []

    for ev in events:
        start: datetime = ev['start']
        end: datetime = ev['end']
        
        # Normalize timezone comparison if needed
        if start.tzinfo and reference_dt.tzinfo:
            local_start = start.astimezone(reference_dt.tzinfo)
            local_end = end.astimezone(reference_dt.tzinfo)
        else:
            local_start = start
            local_end = end

        ev_date = local_start.date()

        include = False
        if scope == 'today':
            include = (ev_date == ref_date)
        elif scope == 'today_remaining':
            include = (ev_date == ref_date) and (ev['is_all_day'] or local_end >= reference_dt)
        elif scope == 'tomorrow':
            include = (ev_date == tomorrow_date)
        elif scope == 'next_24h':
            include = (reference_dt <= local_start <= reference_dt + timedelta(hours=24)) or (local_start <= reference_dt <= local_end)
        elif scope == 'all_upcoming':
            include = (ev['is_all_day'] and ev_date >= ref_date) or (local_end >= reference_dt)
        else:
            include = (ev['is_all_day'] and ev_date >= ref_date) or (local_end >= reference_dt)

        if include:
            # Format time string
            if ev['is_all_day']:
                time_str = 'Ganztägig'
            else:
                time_str = local_start.strftime('%H:%M')
                if local_end and local_end != local_start:
                    time_str += f" - {local_end.strftime('%H:%M')}"

            # Format date badge
            if ev_date == ref_date:
                date_label = 'Heute'
            elif ev_date == tomorrow_date:
                date_label = 'Morgen'
            else:
                german_weekdays = ['Mo', 'Di', 'Mi', 'Do', 'Fr', 'Sa', 'So']
                wd = german_weekdays[ev_date.weekday()]
                date_label = f"{wd}, {ev_date.day}.{ev_date.month}."

            filtered.append({
                'title': ev['title'],
                'time': time_str,
                'start_iso': local_start.isoformat(),
                'end_iso': local_end.isoformat(),
                'date_label': date_label,
                'location': ev.get('location', ''),
                'is_all_day': ev['is_all_day'],
                '_sort_key': local_start
            })

    # Sort chronologically
    filtered.sort(key=lambda x: x['_sort_key'])

    # Truncate and strip internal keys
    result = []
    for item in filtered[:max_events]:
        del item['_sort_key']
        result.append(item)

    return result


def fetch_ical_url(url: str, timeout: int = 8) -> str:
    """Fetch raw iCal content from a URL using urllib."""
    req = urllib.request.Request(
        url,
        headers={'User-Agent': 'KoboDashboardBackend/1.0'}
    )
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        return resp.read().decode('utf-8', errors='replace')
