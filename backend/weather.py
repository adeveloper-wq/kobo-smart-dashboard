"""
Lean, zero-dependency Weather and Humidity client using only Python standard library.
Uses Open-Meteo free API (no API key required, global coverage) with fallback mock data.
"""

import json
import urllib.request
import urllib.parse
from typing import Dict, Any, List, Optional
from datetime import datetime


# Map WMO weather codes to conditions and icons used by Kobo dashboard
WMO_CODE_MAP = {
    0: ("sunny", "Clear sky"),
    1: ("sunny", "Mainly clear"),
    2: ("partly_cloudy", "Partly cloudy"),
    3: ("cloudy", "Overcast"),
    45: ("fog", "Foggy"),
    48: ("fog", "Depositing rime fog"),
    51: ("rainy", "Light drizzle"),
    53: ("rainy", "Moderate drizzle"),
    55: ("rainy", "Dense drizzle"),
    56: ("rainy", "Freezing drizzle"),
    57: ("rainy", "Dense freezing drizzle"),
    61: ("rainy", "Slight rain"),
    63: ("rainy", "Moderate rain"),
    65: ("rainy", "Heavy rain"),
    66: ("rainy", "Freezing rain"),
    67: ("rainy", "Heavy freezing rain"),
    71: ("snowy", "Slight snow fall"),
    73: ("snowy", "Moderate snow fall"),
    75: ("snowy", "Heavy snow fall"),
    77: ("snowy", "Snow grains"),
    80: ("rainy", "Slight rain showers"),
    81: ("rainy", "Moderate rain showers"),
    82: ("rainy", "Violent rain showers"),
    85: ("snowy", "Slight snow showers"),
    86: ("snowy", "Heavy snow showers"),
    95: ("lightning", "Thunderstorm"),
    96: ("lightning", "Thunderstorm with hail"),
    99: ("lightning", "Heavy thunderstorm with hail"),
}


def get_condition_from_wmo(code: int) -> tuple[str, str]:
    """Returns (condition_key, human_description)."""
    return WMO_CODE_MAP.get(code, ("cloudy", "Cloudy"))


def fetch_open_meteo_weather(
    latitude: float,
    longitude: float,
    temperature_unit: str = "celsius",
    timeout: int = 8
) -> Dict[str, Any]:
    """
    Fetch current weather, humidity, and 4-day forecast from Open-Meteo using standard library urllib.
    """
    params = {
        "latitude": latitude,
        "longitude": longitude,
        "current": "temperature_2m,relative_humidity_2m,apparent_temperature,weather_code,wind_speed_10m",
        "daily": "weather_code,temperature_2m_max,temperature_2m_min",
        "timezone": "auto",
        "temperature_unit": temperature_unit
    }
    
    url = f"https://api.open-meteo.com/v1/forecast?{urllib.parse.urlencode(params)}"
    
    req = urllib.request.Request(
        url,
        headers={"User-Agent": "KoboDashboardBackend/1.0"}
    )
    
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        data = json.loads(resp.read().decode("utf-8"))

    current = data.get("current", {})
    daily = data.get("daily", {})

    weather_code = int(current.get("weather_code", 0))
    condition_key, condition_desc = get_condition_from_wmo(weather_code)

    temp = current.get("temperature_2m", 20.0)
    humidity = current.get("relative_humidity_2m", 50.0)
    feels_like = current.get("apparent_temperature", temp)
    wind_speed = current.get("wind_speed_10m", 0.0)

    # Forecast days
    forecast_list: List[Dict[str, Any]] = []
    dates = daily.get("time", [])
    codes = daily.get("weather_code", [])
    max_temps = daily.get("temperature_2m_max", [])
    min_temps = daily.get("temperature_2m_min", [])

    for i in range(min(len(dates), 4)):
        dt_str = dates[i]
        c_code = int(codes[i]) if i < len(codes) else 0
        cond, _ = get_condition_from_wmo(c_code)
        high = max_temps[i] if i < len(max_temps) else temp
        low = min_temps[i] if i < len(min_temps) else temp
        
        forecast_list.append({
            "datetime": dt_str,
            "condition": cond,
            "temperature": high,
            "templow": low
        })

    unit_symbol = "°C" if temperature_unit == "celsius" else "°F"

    # Evaluate humidity comfort level
    if humidity < 35:
        humidity_status = "Dry"
    elif humidity <= 60:
        humidity_status = "Comfortable"
    elif humidity <= 70:
        humidity_status = "Humid"
    else:
        humidity_status = "Very Humid"

    return {
        "condition": condition_key,
        "condition_label": condition_desc,
        "temperature": temp,
        "temperature_unit": unit_symbol,
        "feels_like": feels_like,
        "humidity": humidity,
        "humidity_status": humidity_status,
        "wind_speed": wind_speed,
        "wind_unit": "km/h",
        "forecast": forecast_list
    }


def get_mock_weather(temperature_unit: str = "celsius") -> Dict[str, Any]:
    """Fallback weather when offline or testing."""
    unit = "°C" if temperature_unit == "celsius" else "°F"
    today = datetime.now().strftime("%Y-%m-%d")
    return {
        "condition": "partly_cloudy",
        "condition_label": "Partly Cloudy",
        "temperature": 21.5,
        "temperature_unit": unit,
        "feels_like": 21.0,
        "humidity": 52.0,
        "humidity_status": "Comfortable",
        "wind_speed": 12.0,
        "wind_unit": "km/h",
        "forecast": [
            {"datetime": today, "condition": "partly_cloudy", "temperature": 22.0, "templow": 14.0},
            {"datetime": "2026-11-11", "condition": "sunny", "temperature": 23.0, "templow": 15.0},
            {"datetime": "2026-11-12", "condition": "rainy", "temperature": 18.0, "templow": 12.0},
            {"datetime": "2026-11-13", "condition": "cloudy", "temperature": 19.0, "templow": 13.0},
        ]
    }
