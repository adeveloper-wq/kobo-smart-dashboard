use crate::backend::{EntityState, ForecastItem};
use crate::display::{Canvas, TextAlign};
use crate::i18n;
use crate::ui::layout::{InteractiveTarget, Rect, UiAction};
use crate::ui::theme::Theme;
use crate::ui::widgets::{draw_card, type_scale, PAD};
use chrono::Datelike;

pub fn condition_label(condition: &str) -> String {
    if condition == "unavailable" {
        return i18n::t("state.unavailable");
    }
    i18n::try_t(&format!("weather.condition.{}", condition)).unwrap_or_else(|| condition.to_string())
}

fn day_label(datetime: &str) -> String {
    match datetime.get(..10).and_then(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()) {
        Some(date) if date == chrono::Local::now().date_naive() => i18n::t("date.today"),
        Some(date) => i18n::nth("date.weekdays_short", date.weekday().num_days_from_monday() as usize),
        None => datetime.to_string(),
    }
}

/// Low card: only the 4-day forecast (today included), spread over the full width.
fn render_weather_compact(canvas: &mut Canvas, theme: &Theme, rect: Rect, state: Option<&EntityState>, _forecast_days: usize) {
    let pad = 16.0;
    let x0 = rect.x + pad;
    let y0 = rect.y + pad;
    let inner_h = rect.h - 2.0 * pad;

    let forecast: Vec<ForecastItem> = state
        .and_then(|s| s.attributes.get("forecast"))
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();
    let count = forecast.len().min(4);
    if count == 0 {
        return;
    }
    let col_w = (rect.w - 2.0 * pad) / count as f32;
    for (i, item) in forecast.iter().take(count).enumerate() {
        let cx = x0 + col_w * (i as f32 + 0.5);
        let day = day_label(&item.datetime);
        let color = if i == 0 { theme.text } else { theme.text_muted };
        canvas.draw_text(&day, cx, y0, 30.0, true, color, TextAlign::Center);
        canvas.draw_weather_art(&item.condition, cx - 26.0, y0 + 34.0, 52.0, theme.weather);
        let hi = format!("{:.0}°", item.temperature);
        let lo = item.templow.map(|l| format!("{:.0}°", l));
        let hi_w = canvas.measure_text(&hi, 34.0, true).0;
        let lo_w = lo.as_ref().map(|l| canvas.measure_text(l, 28.0, false).0 + 8.0).unwrap_or(0.0);
        let start = cx - (hi_w + lo_w) / 2.0;
        let ty = y0 + inner_h - 36.0;
        canvas.draw_text(&hi, start, ty, 34.0, true, theme.text, TextAlign::Left);
        if let Some(lo) = lo {
            canvas.draw_text(&lo, start + hi_w + 8.0, ty + 3.0, 28.0, false, theme.text_faint, TextAlign::Left);
        }
    }
}

pub fn render_weather_card(
    canvas: &mut Canvas,
    theme: &Theme,
    rect: Rect,
    _entity_id: &str,
    state: Option<&EntityState>,
    custom_title: Option<&str>,
    forecast_days: usize,
    targets: &mut Vec<InteractiveTarget>,
) {
    draw_card(canvas, theme, rect, None);

    if rect.h < 190.0 {
        render_weather_compact(canvas, theme, rect, state, forecast_days);
        return;
    }

    let condition = state.map(|s| s.state.as_str()).unwrap_or("unavailable");
    let temp = state.and_then(|s| s.get_attr_f32("temperature"));
    let unit = state.and_then(|s| s.get_attr_str("temperature_unit")).unwrap_or("°C");
    let humidity = state.and_then(|s| s.get_attr_f32("humidity"));
    let wind = state.and_then(|s| s.get_attr_f32("wind_speed"));
    let wind_unit = match state.and_then(|s| s.get_attr_str("wind_speed_unit")) {
        Some("km/h") | None => i18n::t("weather.wind_kmh"),
        Some(other) => other.to_string(),
    };

    let compact = rect.h < 260.0;
    let wide = rect.w >= 640.0;
    let x0 = rect.x + PAD;
    let y0 = rect.y + PAD;
    let art = if compact { 68.0 } else if wide { 116.0 } else { 96.0 };

    canvas.draw_weather_art(condition, x0, y0, art, theme.weather);

    let tx = x0 + art + 22.0;
    let temp_size = if compact { 64.0 } else if wide { type_scale::HERO } else { 72.0 };
    let temp_str = temp.map(|t| format!("{:.0}°", t)).unwrap_or_else(|| "--".to_string());
    canvas.draw_text(&temp_str, tx, y0 - 6.0, temp_size, true, theme.text, TextAlign::Left);
    let temp_w = canvas.measure_text(&temp_str, temp_size, true).0;
    if unit != "°C" {
        canvas.draw_text(unit, tx + temp_w + 4.0, y0 + 6.0, 26.0, false, theme.text_muted, TextAlign::Left);
    }

    let mut metrics: Vec<(&str, String)> = Vec::new();
    if let Some(h) = humidity {
        metrics.push(("droplet", i18n::tf("meta.percent", &[("value", &format!("{:.0}", h))])));
    }
    if let Some(w) = wind {
        metrics.push(("wind", format!("{:.0} {}", w, wind_unit)));
    }

    let _ = custom_title;
    let cond_text = condition_label(condition);
    let cond_text = cond_text.as_str();

    if compact {
        // Condition next to the temperature, humidity/wind on the right
        let metrics_w = 150.0;
        let cx = tx + temp_w + 18.0;
        canvas.draw_text_fit(
            cond_text,
            cx,
            y0 + temp_size * 0.28,
            30.0,
            true,
            theme.text,
            TextAlign::Left,
            rect.x + rect.w - PAD - metrics_w - cx,
        );
        let mut my = y0 + 2.0;
        for (icon, text) in &metrics {
            let right = rect.x + rect.w - PAD;
            let tw = canvas.measure_text(text, 24.0, true).0;
            canvas.draw_text(text, right, my, 24.0, true, theme.text, TextAlign::Right);
            canvas.draw_icon(icon, right - tw - 36.0, my - 1.0, 26.0, theme.text_muted, 2.0);
            my += 34.0;
        }
    } else if wide {
        let metrics_w = 210.0;
        let cond_size = 32.0;
        canvas.draw_text_fit(
            cond_text,
            tx,
            y0 + temp_size * 0.95,
            cond_size,
            true,
            theme.text,
            TextAlign::Left,
            rect.x + rect.w - PAD - metrics_w - tx,
        );
        // Metrics column, right aligned
        let mut my = y0 + 6.0;
        for (icon, text) in &metrics {
            let right = rect.x + rect.w - PAD;
            let tw = canvas.measure_text(text, type_scale::BODY, true).0;
            canvas.draw_text(text, right, my, type_scale::BODY, true, theme.text, TextAlign::Right);
            canvas.draw_icon(icon, right - tw - 40.0, my - 1.0, 30.0, theme.text_muted, 2.0);
            my += 46.0;
        }
    } else {
        let max_w = rect.x + rect.w - PAD - tx;
        canvas.draw_text_fit(cond_text, tx, y0 + temp_size * 0.92, 28.0, true, theme.text, TextAlign::Left, max_w);
        let line = metrics.iter().map(|(_, t)| t.as_str()).collect::<Vec<_>>().join("  ·  ");
        canvas.draw_text_fit(&line, tx, y0 + temp_size * 0.92 + 34.0, 24.0, false, theme.text_muted, TextAlign::Left, max_w);
    }

    // Forecast strip anchored to the bottom of the card
    let forecast: Vec<ForecastItem> = state
        .and_then(|s| s.attributes.get("forecast"))
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    let fc_art = if compact { 36.0 } else { 48.0 };
    let strip_h = if compact { 24.0 + 36.0 + 4.0 + 28.0 } else { 22.0 + 6.0 + 46.0 + 6.0 + 28.0 };
    let strip_y = rect.y + rect.h - PAD - strip_h;
    let divider_y = strip_y - 16.0;
    canvas.draw_line(x0, divider_y, rect.x + rect.w - PAD, divider_y, 2.0, theme.border);

    if forecast.is_empty() {
        canvas.draw_text(
            &i18n::t("weather.no_forecast"),
            rect.x + rect.w / 2.0,
            strip_y + strip_h / 2.0 - 12.0,
            type_scale::BODY,
            false,
            theme.text_faint,
            TextAlign::Center,
        );
    } else {
        let count = forecast.len().min(forecast_days.max(1));
        let col_w = (rect.w - 2.0 * PAD) / count as f32;
        for (i, item) in forecast.iter().take(count).enumerate() {
            let cx = x0 + col_w * (i as f32 + 0.5);
            let day = day_label(&item.datetime);
            let day_color = if i == 0 { theme.text } else { theme.text_muted };
            canvas.draw_text(&day, cx, strip_y, 24.0, true, day_color, TextAlign::Center);
            canvas.draw_weather_art(&item.condition, cx - fc_art / 2.0, strip_y + if compact { 24.0 } else { 26.0 }, fc_art, theme.weather);

            let (hi_size, lo_size, sep) = if col_w < 130.0 { (24.0, 22.0, 6.0) } else { (28.0, 24.0, 10.0) };
            let hi = format!("{:.0}°", item.temperature);
            let lo = item.templow.map(|l| format!("{:.0}°", l));
            let hi_w = canvas.measure_text(&hi, hi_size, true).0;
            let lo_w = lo.as_ref().map(|l| canvas.measure_text(l, lo_size, false).0 + sep).unwrap_or(0.0);
            let start = cx - (hi_w + lo_w) / 2.0;
            let ty = strip_y + if compact { 64.0 } else { 80.0 } + (28.0 - hi_size) * 0.6;
            canvas.draw_text(&hi, start, ty, hi_size, true, theme.text, TextAlign::Left);
            if let Some(lo) = lo {
                canvas.draw_text(&lo, start + hi_w + sep, ty + 1.0, lo_size, false, theme.text_faint, TextAlign::Left);
            }
        }
    }

    let _ = targets;
}
