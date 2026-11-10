use crate::display::{Canvas, TextAlign};
use crate::ui::layout::{InteractiveTarget, Rect};
use crate::ui::theme::Theme;
use crate::ui::widgets::{draw_card, draw_icon_badge, type_scale, PAD};

pub fn render_greeting_card(
    canvas: &mut Canvas,
    theme: &Theme,
    rect: Rect,
    title: Option<&str>,
    subtitle: Option<&str>,
    icon: Option<&str>,
    _targets: &mut Vec<InteractiveTarget>,
) {
    draw_card(canvas, theme, rect, None);

    let x0 = rect.x + PAD;
    let y0 = rect.y + PAD;
    let badge_d = (rect.h - 2.0 * PAD).clamp(56.0, 72.0);

    let icon_name = icon.unwrap_or("sun");
    let tone = match icon_name {
        "moon" => theme.media,
        "sun" => theme.warm,
        "cloud" => theme.cool,
        _ => theme.accent,
    };

    draw_icon_badge(canvas, theme, x0, y0 + (rect.h - 2.0 * PAD - badge_d) / 2.0, badge_d, icon_name, tone, false);

    let tx = x0 + badge_d + 20.0;
    let max_w = rect.x + rect.w - PAD - tx;

    let title_text = title.unwrap_or("Welcome");
    let sub_text = subtitle.unwrap_or("");

    let title_size = if rect.w >= 600.0 { type_scale::TITLE + 4.0 } else { type_scale::TITLE };
    canvas.draw_text_fit(title_text, tx, y0 + 4.0, title_size, true, theme.text, TextAlign::Left, max_w);

    if !sub_text.is_empty() {
        canvas.draw_text_fit(sub_text, tx, y0 + title_size + 14.0, type_scale::BODY, false, theme.text_muted, TextAlign::Left, max_w);
    }
}
