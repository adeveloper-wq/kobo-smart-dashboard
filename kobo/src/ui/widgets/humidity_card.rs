use crate::config::HumiditySensorItem;
use crate::display::{Canvas, TextAlign};
use crate::ui::layout::{InteractiveTarget, Rect};
use crate::ui::theme::Theme;
use crate::ui::widgets::{draw_card, draw_icon_badge, draw_value_unit, type_scale, PAD};

pub fn render_humidity_card(
    canvas: &mut Canvas,
    theme: &Theme,
    rect: Rect,
    title: Option<&str>,
    value: &str,
    unit: &str,
    status: &str,
    _targets: &mut Vec<InteractiveTarget>,
) {
    draw_card(canvas, theme, rect, None);

    let x0 = rect.x + PAD;
    let y0 = rect.y + PAD;
    let badge_d = 52.0;

    draw_icon_badge(canvas, theme, x0, y0, badge_d, "droplet", theme.cool, false);

    let title_text = title.unwrap_or("Luftfeuchtigkeit");
    canvas.draw_text(title_text, x0 + badge_d + 16.0, y0 + 12.0, type_scale::LABEL, true, theme.text_muted, TextAlign::Left);

    let val_y = y0 + badge_d + 14.0;
    let max_w = rect.w - 2.0 * PAD;
    draw_value_unit(canvas, theme, x0, val_y, value, unit, type_scale::HERO * 0.75, max_w);

    if !status.is_empty() {
        let status_y = val_y + 54.0;
        canvas.draw_text_fit(status, x0, status_y, type_scale::BODY, false, theme.text_muted, TextAlign::Left, max_w);
    }
}

pub fn render_humidity_row(
    canvas: &mut Canvas,
    theme: &Theme,
    rect: Rect,
    sensors: &[HumiditySensorItem],
) {
    draw_card(canvas, theme, rect, None);
    if sensors.is_empty() {
        return;
    }

    let count = sensors.len().min(5);
    let icon_col_w = 58.0;
    let icon_s = 32.0;
    canvas.draw_icon(
        "droplet",
        rect.x + (icon_col_w - icon_s) / 2.0 + 4.0,
        rect.y + (rect.h - icon_s) / 2.0,
        icon_s,
        theme.cool.solid,
        2.2,
    );

    let col_x0 = rect.x + icon_col_w;
    let col_w = (rect.w - icon_col_w) / count as f32;
    let name_size = 24.0;
    let val_size = 46.0;
    let unit_size = 24.0;

    for (i, sensor) in sensors.iter().take(count).enumerate() {
        let cx = col_x0 + (i as f32 + 0.5) * col_w;
        if i > 0 {
            let div_x = col_x0 + i as f32 * col_w;
            canvas.fill_rect(div_x - 1.0, rect.y + 18.0, 2.0, rect.h - 36.0, theme.border);
        }

        let max_w = col_w - 20.0;
        let name_y = rect.y + 18.0;
        canvas.draw_text_fit(&sensor.name, cx, name_y, name_size, true, theme.text_muted, TextAlign::Center, max_w);

        let val_w = canvas.measure_text(&sensor.value, val_size, true).0;
        let unit_w = if sensor.unit.is_empty() { 0.0 } else { canvas.measure_text(&sensor.unit, unit_size, false).0 + 4.0 };
        let total_w = (val_w + unit_w).min(max_w);
        let start_x = cx - total_w / 2.0;
        let val_y = rect.y + 58.0;
        let drawn_vw = canvas.draw_text_fit(&sensor.value, start_x, val_y, val_size, true, theme.text, TextAlign::Left, (max_w - unit_w).max(10.0));
        if !sensor.unit.is_empty() {
            canvas.draw_text(&sensor.unit, start_x + drawn_vw + 4.0, val_y + val_size * 0.85 - unit_size * 0.85, unit_size, false, theme.text_muted, TextAlign::Left);
        }
    }
}
