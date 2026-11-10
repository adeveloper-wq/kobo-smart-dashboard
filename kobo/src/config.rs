use serde::{Deserialize, Serialize};
use crate::display::Rotation;
use crate::models;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub device: DeviceConfig,
    #[serde(default)]
    pub backend: BackendConfig,
    #[serde(default)]
    pub theme: ThemeConfig,
    #[serde(default)]
    pub pages: Vec<PageConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendConfig {
    #[serde(default = "default_backend_url")]
    pub url: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_backend_url() -> String {
    "http://127.0.0.1:8000".to_string()
}

impl Default for BackendConfig {
    fn default() -> Self {
        Self {
            url: default_backend_url(),
            enabled: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceConfig {
    #[serde(default = "default_auto")]
    pub model: String,
    #[serde(default = "default_orientation")]
    pub orientation: String,
    #[serde(default)]
    pub width: u32,
    #[serde(default)]
    pub height: u32,
    #[serde(default = "default_touch_device")]
    pub touch_device: String,
    #[serde(default = "default_fb_device")]
    pub fb_device: String,
    #[serde(default = "default_full_refresh_cycles")]
    pub full_refresh_cycles: u32,
    #[serde(default = "default_true")]
    pub swap_touch_xy: bool,
    #[serde(default = "default_true")]
    pub invert_touch_x: bool,
    #[serde(default)]
    pub invert_touch_y: bool,
    #[serde(default = "default_auto")]
    pub fb_rotation: String,
    #[serde(default = "default_landscape_rotation")]
    pub landscape_rotation: i32,
    #[serde(default = "default_true")]
    pub color_panel: bool,
    #[serde(default)]
    pub frontlight_timeout_secs: u32,
    #[serde(default)]
    pub sleep_after_secs: u32,
    #[serde(default = "default_sleep_wake_secs")]
    pub sleep_wake_secs: u32,
}

fn default_sleep_wake_secs() -> u32 {
    900
}
fn default_orientation() -> String {
    "portrait".to_string()
}
fn default_touch_device() -> String {
    "auto".to_string()
}
fn default_fb_device() -> String {
    "/dev/fb0".to_string()
}
fn default_full_refresh_cycles() -> u32 {
    10
}
fn default_auto() -> String {
    "auto".to_string()
}
fn default_landscape_rotation() -> i32 {
    90
}

impl Default for DeviceConfig {
    fn default() -> Self {
        Self {
            model: default_auto(),
            orientation: default_orientation(),
            width: 0,
            height: 0,
            touch_device: default_touch_device(),
            fb_device: default_fb_device(),
            full_refresh_cycles: default_full_refresh_cycles(),
            swap_touch_xy: true,
            invert_touch_x: true,
            invert_touch_y: false,
            fb_rotation: default_auto(),
            landscape_rotation: default_landscape_rotation(),
            color_panel: true,
            frontlight_timeout_secs: 0,
            sleep_after_secs: 0,
            sleep_wake_secs: default_sleep_wake_secs(),
        }
    }
}

impl DeviceConfig {
    pub fn get_dimensions(&self) -> (u32, u32) {
        self.resolve_dimensions(None)
    }

    pub fn resolve_dimensions(&self, probed: Option<(u32, u32)>) -> (u32, u32) {
        if self.width > 0 && self.height > 0 {
            return (self.width, self.height);
        }
        let (pw, ph) = probed
            .map(|(a, b)| (a.min(b), a.max(b)))
            .or_else(|| models::by_key(&self.model).map(|m| (m.width, m.height)))
            .unwrap_or((1072, 1448));
        if self.is_landscape() {
            (ph, pw)
        } else {
            (pw, ph)
        }
    }

    pub fn is_landscape(&self) -> bool {
        self.orientation.to_lowercase() == "landscape"
    }

    pub fn canvas_rotation(&self) -> Rotation {
        if self.is_landscape() {
            Rotation::from_degrees(self.landscape_rotation)
        } else {
            Rotation::R0
        }
    }

    pub fn fb_rotation(&self) -> Option<Rotation> {
        self.fb_rotation.trim().parse::<i32>().ok().map(Rotation::from_degrees)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeConfig {
    #[serde(default = "default_color_mode")]
    pub mode: String,
    #[serde(default = "default_accent_color")]
    pub accent_color: String,
    #[serde(default = "default_card_corner_radius")]
    pub card_corner_radius: f32,
    #[serde(default = "default_time_format")]
    pub time_format: String,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub utc_offset: Option<String>,
}

fn default_color_mode() -> String {
    "kaleido".to_string()
}
fn default_accent_color() -> String {
    "#D97706".to_string()
}
fn default_card_corner_radius() -> f32 {
    18.0
}
fn default_time_format() -> String {
    "24h".to_string()
}
fn default_language() -> String {
    "de".to_string()
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            mode: default_color_mode(),
            accent_color: default_accent_color(),
            card_corner_radius: default_card_corner_radius(),
            time_format: default_time_format(),
            language: default_language(),
            utc_offset: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageConfig {
    pub id: String,
    pub title: String,
    #[serde(default = "default_page_icon")]
    pub icon: String,
    #[serde(default)]
    pub cards: Vec<CardConfig>,
}

fn default_page_icon() -> String {
    "home".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum CardConfig {
    #[serde(rename = "weather")]
    Weather {
        #[serde(default = "default_weather_entity")]
        entity_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(default = "default_forecast_days")]
        forecast_days: usize,
    },
    #[serde(rename = "greeting")]
    Greeting {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        subtitle: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        icon: Option<String>,
    },
    #[serde(rename = "calendar")]
    Calendar {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(default)]
        events: Vec<CalendarEventItem>,
    },
    #[serde(rename = "humidity")]
    Humidity {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(default = "default_humidity_val")]
        value: String,
        #[serde(default = "default_percent")]
        unit: String,
        #[serde(default)]
        status: String,
    },
}

fn default_weather_entity() -> String {
    "weather.home".to_string()
}
fn default_forecast_days() -> usize {
    4
}
fn default_true() -> bool {
    true
}
fn default_humidity_val() -> String {
    "50".to_string()
}
fn default_percent() -> String {
    "%".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarEventItem {
    pub title: String,
    pub time: String,
    #[serde(default)]
    pub date_label: Option<String>,
    #[serde(default)]
    pub location: Option<String>,
    #[serde(default)]
    pub is_all_day: bool,
}

fn default_version() -> u32 {
    1
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: default_version(),
            device: DeviceConfig::default(),
            backend: BackendConfig::default(),
            theme: ThemeConfig::default(),
            pages: Vec::new(),
        }
    }
}

impl AppConfig {
    pub fn load_or_default(path: &Path) -> Self {
        if path.exists() {
            match std::fs::read_to_string(path) {
                Ok(content) => match serde_yaml::from_str(&content) {
                    Ok(cfg) => return cfg,
                    Err(e) => tracing::error!("Failed to parse {:?}: {}", path, e),
                },
                Err(e) => tracing::error!("Failed to read {:?}: {}", path, e),
            }
        }
        Self::default()
    }
}
