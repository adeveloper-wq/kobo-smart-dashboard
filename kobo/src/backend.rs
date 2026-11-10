use crate::config::{AppConfig, CardConfig, PageConfig, ThemeConfig};
use crate::ha::models::EntityState;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendSleepInfo {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_sleep_secs")]
    pub sleep_wake_secs: u32,
    #[serde(default)]
    pub next_update_at: Option<String>,
    #[serde(default)]
    pub sleep_note: Option<String>,
}

fn default_true() -> bool {
    true
}

fn default_sleep_secs() -> u32 {
    1800
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendDashboardPayload {
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub generated_at: String,
    #[serde(default)]
    pub theme: Option<ThemeConfig>,
    #[serde(default)]
    pub greeting: Option<serde_json::Value>,
    #[serde(default)]
    pub weather: Option<serde_json::Value>,
    #[serde(default)]
    pub calendar: Option<serde_json::Value>,
    #[serde(default)]
    pub cards: Vec<CardConfig>,
    #[serde(default)]
    pub sleep: Option<BackendSleepInfo>,
}

#[derive(Clone)]
pub struct BackendClient {
    url: String,
    client: reqwest::Client,
}

impl BackendClient {
    pub fn new(url: &str) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_default();
        Self {
            url: url.trim_end_matches('/').to_string(),
            client,
        }
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub async fn fetch_dashboard(&self) -> Result<BackendDashboardPayload, Box<dyn std::error::Error + Send + Sync>> {
        let endpoint = format!("{}/api/dashboard", self.url);
        let resp = self.client.get(&endpoint).send().await?;
        let payload = resp.json::<BackendDashboardPayload>().await?;
        Ok(payload)
    }

    /// Converts backend payload into states map and pages for the renderer
    pub fn apply_to_app(payload: &BackendDashboardPayload, config: &mut AppConfig) -> HashMap<String, EntityState> {
        let mut states = HashMap::new();

        // If backend provided theme, override theme
        if let Some(t) = &payload.theme {
            config.theme = t.clone();
        }

        // Populate Weather entity state for render_weather_card
        if let Some(w) = &payload.weather {
            let mut attrs = HashMap::new();
            if let Some(temp) = w.get("temperature").and_then(|v| v.as_f64()) {
                attrs.insert("temperature".to_string(), serde_json::json!(temp));
            }
            if let Some(unit) = w.get("temperature_unit").and_then(|v| v.as_str()) {
                attrs.insert("temperature_unit".to_string(), serde_json::json!(unit));
            }
            if let Some(h) = w.get("humidity").and_then(|v| v.as_f64()) {
                attrs.insert("humidity".to_string(), serde_json::json!(h));
            }
            if let Some(w_spd) = w.get("wind_speed").and_then(|v| v.as_f64()) {
                attrs.insert("wind_speed".to_string(), serde_json::json!(w_spd));
            }
            if let Some(fc) = w.get("forecast") {
                attrs.insert("forecast".to_string(), fc.clone());
            }

            let cond = w.get("condition").and_then(|v| v.as_str()).unwrap_or("partly_cloudy");
            states.insert(
                "weather.home".to_string(),
                EntityState {
                    entity_id: "weather.home".to_string(),
                    state: cond.to_string(),
                    attributes: attrs,
                    last_changed: payload.generated_at.clone(),
                },
            );
        }

        // Replace pages with a single page containing the backend-defined cards
        config.pages = vec![PageConfig {
            id: "backend_main".to_string(),
            title: "Today".to_string(),
            icon: "home".to_string(),
            cards: payload.cards.clone(),
        }];

        states
    }
}
