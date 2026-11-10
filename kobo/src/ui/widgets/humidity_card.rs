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

    let title_text = title.unwrap_or("Humidity");
    canvas.draw_text(title_text, x0 + badge_d + 16.0, y0 + 12.0, type_scale::LABEL, true, theme.text_muted, TextAlign::Left);

    let val_y = y0 + badge_d + 14.0;
    let max_w = rect.w - 2.0 * PAD;
    draw_value_unit(canvas, theme, x0, val_y, value, unit, type_scale::HERO * 0.75, max_w);

    if !status.is_empty() {
        let status_y = val_y + 54.0;
        canvas.draw_text_fit(status, x0, status_y, type_scale::BODY, false, theme.text_muted, TextAlign::Left, max_w);
    }
}
