use crate::backend::EntityState;
use crate::config::{AppConfig, CardConfig};
use crate::display::canvas::ui_size;
use crate::display::Canvas;
use crate::ui::layout::{InteractiveTarget, Rect, UiAction};
use crate::ui::theme::Theme;
use crate::ui::widgets::header::{read_device_status, HeaderStatus, SIDE_PAD};
use crate::ui::widgets::*;
use chrono::{DateTime, Utc};
use std::collections::HashMap;

const GAP: f32 = 18.0;
const BOTTOM_PAD: f32 = 28.0;
/// Space kept at the bottom for the sleep-mode note
const SLEEP_NOTE_H: f32 = 124.0;
const LANDSCAPE_LEFT_W: f32 = 440.0;
const LANDSCAPE_MIN_HALF_W: f32 = 300.0;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Span {
    Full,
    Half,
}

fn span_of(card: &CardConfig) -> Span {
    match card {
        CardConfig::Weather { .. } | CardConfig::Greeting { .. } | CardConfig::Calendar { .. } | CardConfig::HumidityRow { .. } => Span::Full,
        CardConfig::Humidity { .. } => Span::Half,
    }
}

/// (natural, minimum) height of a card rendered at width `w`.
fn heights(card: &CardConfig, w: f32) -> (f32, f32) {
    match card {
        CardConfig::Weather { .. } => {
            if w >= 640.0 {
                (312.0, 300.0)
            } else {
                (318.0, 306.0)
            }
        }
        CardConfig::Greeting { .. } => (140.0, 100.0),
        CardConfig::Calendar { events, .. } => {
            let row_count = events.len().clamp(1, 7);
            let nat = 72.0 + (row_count as f32) * 48.0;
            (nat, (nat - 30.0).max(120.0))
        }
        CardConfig::Humidity { .. } => (176.0, 140.0),
        CardConfig::HumidityRow { .. } => (136.0, 110.0),
    }
}

fn layout_zone(cards: &[&CardConfig], x: f32, y: f32, w: f32, avail_h: f32) -> Vec<(usize, Rect)> {
    let mut rows: Vec<Vec<usize>> = Vec::new();
    let mut pending: Option<usize> = None;
    for (i, card) in cards.iter().enumerate() {
        match span_of(card) {
            Span::Full => {
                if let Some(p) = pending.take() {
                    rows.push(vec![p]);
                }
                rows.push(vec![i]);
            }
            Span::Half => match pending.take() {
                Some(p) => rows.push(vec![p, i]),
                None => pending = Some(i),
            },
        }
    }
    if let Some(p) = pending {
        rows.push(vec![p]);
    }

    let half_w = (w - GAP) / 2.0;
    let row_heights: Vec<(f32, f32)> = rows
        .iter()
        .map(|row| {
            let item_w = if row.len() == 2 { half_w } else { w };
            row.iter()
                .map(|&i| heights(cards[i], item_w))
                .fold((0.0f32, 0.0f32), |(n, m), (cn, cm)| (n.max(cn), m.max(cm)))
        })
        .collect();

    let gaps = GAP * rows.len().saturating_sub(1) as f32;
    let natural: f32 = row_heights.iter().map(|h| h.0).sum::<f32>() + gaps;
    let slack: f32 = row_heights.iter().map(|h| h.0 - h.1).sum();
    let k = if natural > avail_h && slack > 0.0 { ((natural - avail_h) / slack).min(1.0) } else { 0.0 };

    let mut out = Vec::new();
    let mut cur_y = y;
    for (row, (nat, min)) in rows.iter().zip(row_heights) {
        let h = nat - k * (nat - min);
        if cur_y + h > y + avail_h + 1.0 {
            break;
        }
        if row.len() == 2 {
            out.push((row[0], Rect::new(x, cur_y, half_w, h)));
            out.push((row[1], Rect::new(x + half_w + GAP, cur_y, half_w, h)));
        } else {
            out.push((row[0], Rect::new(x, cur_y, w, h)));
        }
        cur_y += h + GAP;
    }
    out
}

pub const HEADER_WITH_TABS: f32 = 206.0;
pub const HEADER_WITHOUT_TABS: f32 = 122.0;

fn zones_for<'a>(cards: &'a [CardConfig], cw: f32, ch: f32, top: f32) -> Vec<(&'a CardConfig, Rect)> {
    let avail_h = ch - top - BOTTOM_PAD;
    let content_w = cw - 2.0 * SIDE_PAD;
    let half_w = (content_w - GAP) / 2.0;

    // Separate into top cards (greeting, weather, humidity), calendar cards (for both persons), and optional humidity_row
    let (cals, rest): (Vec<&CardConfig>, Vec<&CardConfig>) =
        cards.iter().partition(|c| matches!(c, CardConfig::Calendar { .. }));
    let (humidity_rows, non_cals): (Vec<&CardConfig>, Vec<&CardConfig>) =
        rest.into_iter().partition(|c| matches!(c, CardConfig::HumidityRow { .. }));

    // When we have 2 calendars and horizontal layout:
    // Display calendars for both persons side by side in separate columns!
    if cals.len() == 2 && !non_cals.is_empty() {
        let has_hum_row = !humidity_rows.is_empty();
        let usable_h = (avail_h - GAP).max(100.0);
        // Compact top row (greeting + weather); calendars get the remaining height (minus optional humidity row)
        let top_h = (usable_h * 0.2).clamp(145.0, 170.0);
        let hum_h = if has_hum_row { 130.0 } else { 0.0 };
        let cal_h = (usable_h - top_h - if has_hum_row { hum_h + GAP } else { 0.0 }).max(120.0);
        let cal_y = top + top_h + GAP;

        let mut out = Vec::new();

        // Top Row: non_cals (Greeting + Weather with forecast)
        if non_cals.len() == 2 {
            out.push((non_cals[0], Rect::new(SIDE_PAD, top, half_w, top_h)));
            out.push((non_cals[1], Rect::new(SIDE_PAD + half_w + GAP, top, half_w, top_h)));
        } else if non_cals.len() == 1 {
            out.push((non_cals[0], Rect::new(SIDE_PAD, top, content_w, top_h)));
        } else {
            for (idx, r) in layout_zone(&non_cals, SIDE_PAD, top, content_w, top_h) {
                out.push((non_cals[idx], r));
            }
        }

        // Middle/Bottom Row: Both calendars side by side in separate columns!
        out.push((cals[0], Rect::new(SIDE_PAD, cal_y, half_w, cal_h)));
        out.push((cals[1], Rect::new(SIDE_PAD + half_w + GAP, cal_y, half_w, cal_h)));

        // Optional 3rd Row: 5-sensor humidity strip
        if let Some(hum_card) = humidity_rows.first() {
            let hum_y = cal_y + cal_h + GAP;
            out.push((*hum_card, Rect::new(SIDE_PAD, hum_y, content_w, hum_h)));
        }

        return out;
    }

    // Fallback general 2-column or 1-column layout
    if cw > ch {
        let (left, right): (Vec<&CardConfig>, Vec<&CardConfig>) =
            cards.iter().partition(|c| matches!(c, CardConfig::Weather { .. } | CardConfig::Greeting { .. }));
        if !left.is_empty() && !right.is_empty() {
            let left_w = (content_w * 0.45).clamp(340.0, 560.0);
            let right_x = SIDE_PAD + left_w + GAP;
            let right_w = content_w - left_w - GAP;
            let mut out = Vec::new();
            for (idx, r) in layout_zone(&left, SIDE_PAD, top, left_w, avail_h) {
                out.push((left[idx], r));
            }
            for (idx, r) in layout_zone(&right, right_x, top, right_w, avail_h) {
                out.push((right[idx], r));
            }
            return out;
        }
    }

    let all: Vec<&CardConfig> = cards.iter().collect();
    layout_zone(&all, SIDE_PAD, top, content_w, avail_h).into_iter().map(|(idx, r)| (all[idx], r)).collect()
}

pub struct ScreenRenderer {
    pub theme: Theme,
    pub targets: Vec<InteractiveTarget>,
    pub hidden_cards: usize,
    pub last_update: Option<DateTime<Utc>>,
    pub frontlight: Option<bool>,
    pub sleep_until: Option<String>,
    scale: f32,
}

impl ScreenRenderer {
    pub fn new(config: &AppConfig) -> Self {
        Self {
            theme: Theme::from_config(&config.theme),
            targets: Vec::new(),
            hidden_cards: 0,
            last_update: None,
            frontlight: None,
            sleep_until: None,
            scale: 1.0,
        }
    }

    pub fn render(
        &mut self,
        canvas: &mut Canvas,
        config: &AppConfig,
        active_page_idx: usize,
        states: &HashMap<String, EntityState>,
        is_live: bool,
    ) {
        self.targets.clear();
        self.hidden_cards = 0;
        self.scale = canvas.scale;
        canvas.clear(self.theme.bg);

        let (battery, charging, wifi_up) = read_device_status();
        let wifi_up = crate::ui::widgets::header::shown_wifi(wifi_up, self.sleep_until.is_some(), crate::power::wifi_switched_off());
        let status = HeaderStatus {
            is_live,
            is_demo: false,
            last_update: self.last_update,
            battery,
            charging,
            wifi_up,
            frontlight: self.frontlight,
        };

        let header_bottom = render_header(
            canvas,
            &self.theme,
            &config.theme,
            &config.pages,
            active_page_idx,
            &status,
            &mut self.targets,
        );

        let Some(page) = config.pages.get(active_page_idx) else { return };

        let has_tabs = config.pages.len() > 1;
        let reserve = if self.sleep_until.is_some() && !has_tabs { SLEEP_NOTE_H } else { 0.0 };
        let (w, h) = canvas.ui_size();
        let placements = zones_for(&page.cards, w, h - reserve, header_bottom);
        self.hidden_cards = page.cards.len() - placements.len();

        for (card, rect) in placements {
            self.render_card(canvas, card, rect, states);
        }

        if let Some(next) = self.sleep_until.clone() {
            let bar = if has_tabs {
                Rect::new(SIDE_PAD, HEADER_WITH_TABS - 84.0, w - 2.0 * SIDE_PAD, 64.0)
            } else {
                Rect::new(SIDE_PAD, h - SLEEP_NOTE_H + 4.0, w - 2.0 * SIDE_PAD, SLEEP_NOTE_H - 28.0)
            };
            self.render_sleep_note(canvas, bar, &next);
            self.targets.retain(|t| matches!(t.action, UiAction::ExitToReader));
        }
    }

    fn render_sleep_note(&self, canvas: &mut Canvas, bar: Rect, next: &str) {
        use crate::display::TextAlign;
        use crate::i18n;
        let theme = &self.theme;
        canvas.fill_rounded_rect(bar.x, bar.y, bar.w, bar.h, bar.h / 2.0, theme.text);
        let icon = (bar.h * 0.5).min(46.0);
        let ix = bar.x + bar.h * 0.4;
        canvas.draw_icon("moon", ix, bar.y + (bar.h - icon) / 2.0, icon, theme.surface, 2.2);
        let tx = ix + icon + 18.0;
        let max_w = bar.x + bar.w - bar.h * 0.4 - tx;
        
        let (line, size) = sleep_line(canvas, next, max_w, 30.0);
        let tw = canvas.draw_text_fit(&line, tx, bar.y + bar.h / 2.0 - size / 2.0, size, true, theme.surface, TextAlign::Left, max_w);
        let hint_x = tx + tw + 18.0;
        if hint_x + 120.0 < tx + max_w {
            let muted = theme.surface.mix(theme.text, 0.35);
            canvas.draw_text_fit(&i18n::t("sleep.wake_hint"), hint_x, bar.y + bar.h / 2.0 - 13.0, 24.0, false, muted, TextAlign::Left, tx + max_w - hint_x);
        }
    }

    fn render_card(&mut self, canvas: &mut Canvas, card: &CardConfig, rect: Rect, states: &HashMap<String, EntityState>) {
        let theme = &self.theme;
        let targets = &mut self.targets;
        match card {
            CardConfig::Weather { entity_id, title, forecast_days } => {
                render_weather_card(canvas, theme, rect, entity_id, states.get(entity_id), title.as_deref(), *forecast_days, targets)
            }
            CardConfig::Greeting { title, temperature, condition, humidity, .. } => {
                render_greeting_card(canvas, theme, rect, title.as_deref(), *temperature, condition.as_deref(), *humidity, targets);
            }
            CardConfig::Calendar { title, events } => {
                render_calendar_card(canvas, theme, rect, title.as_deref(), events, targets);
            }
            CardConfig::Humidity { title, value, unit, status } => {
                render_humidity_card(canvas, theme, rect, title.as_deref(), value, unit, status, targets);
            }
            CardConfig::HumidityRow { sensors } => {
                render_humidity_row(canvas, theme, rect, sensors);
            }
        }
    }

    pub fn handle_touch(&self, x: f32, y: f32) -> Option<UiAction> {
        let (x, y) = (x / self.scale, y / self.scale);
        if let Some(t) = self.targets.iter().rev().find(|t| t.rect.contains(x, y)) {
            return Some(t.action.clone());
        }
        let threshold = 28.0;
        self.targets
            .iter()
            .rev()
            .map(|t| (t, t.rect.distance_to(x, y)))
            .filter(|(_, d)| *d <= threshold)
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(t, _)| t.action.clone())
    }
}

fn sleep_line(canvas: &Canvas, next: &str, max_w: f32, max_size: f32) -> (String, f32) {
    let title = crate::i18n::t("sleep.title");
    let full = format!("{} · {}", title, next);
    let mut size = max_size;
    while size >= 22.0 {
        if canvas.measure_text(&full, size, true).0 <= max_w {
            return (full, size);
        }
        size -= 2.0;
    }
    (full, 22.0)
}
