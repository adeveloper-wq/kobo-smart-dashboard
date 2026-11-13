use crate::config::CalendarEventItem;
use crate::display::{Canvas, TextAlign};
use crate::ui::layout::{InteractiveTarget, Rect};
use crate::ui::theme::Theme;
use crate::ui::widgets::{draw_card, type_scale, PAD};

pub fn render_calendar_card(
    canvas: &mut Canvas,
    theme: &Theme,
    rect: Rect,
    title: Option<&str>,
    events: &[CalendarEventItem],
    _targets: &mut Vec<InteractiveTarget>,
) {
    draw_card(canvas, theme, rect, None);

    let x0 = rect.x + PAD;
    let mut y0 = rect.y + PAD;
    let max_w = rect.w - 2.0 * PAD;

    // Card Header Title (enlarged for small display readability)
    let header_title = title.unwrap_or("Termine").to_uppercase();
    let header_size = 30.0;
    canvas.draw_text_fit(&header_title, x0, y0, header_size, true, theme.text, TextAlign::Left, max_w);
    y0 += header_size + 14.0;

    // Divider line
    canvas.fill_rect(x0, y0, max_w, 2.5, theme.border);
    y0 += 16.0;

    if events.is_empty() {
        let msg = "Keine anstehenden Termine";
        canvas.draw_text(msg, x0 + max_w / 2.0, y0 + 24.0, type_scale::BODY, false, theme.text_muted, TextAlign::Center);
        return;
    }

    let time_col_w = if rect.w >= 600.0 { 160.0 } else { 140.0 };
    let avail_h = (rect.y + rect.h - 10.0 - y0).max(0.0);
    let target_rows = events.len().min(7) as f32;
    let row_h = (avail_h / target_rows).clamp(40.0, 60.0);
    let text_scale = (row_h / 60.0).clamp(0.78, 1.0);
    let label_size = type_scale::LABEL * text_scale;
    let body_size = type_scale::BODY * text_scale;
    let text_y_off = ((row_h - body_size) * 0.35).max(2.0);

    for ev in events {
        if y0 + row_h > rect.y + rect.h - 10.0 + 0.5 {
            break;
        }

        // Event date badge (day/month or Heute/Morgen)
        let date_badge = ev.date_label.as_deref().unwrap_or("");
        canvas.draw_text_fit(date_badge, x0, y0 + text_y_off, label_size, true, theme.accent.solid, TextAlign::Left, time_col_w - 12.0);

        // Title and Location
        let title_x = x0 + time_col_w;
        let title_max_w = rect.x + rect.w - PAD - title_x;

        let display_title = if let Some(loc) = &ev.location {
            if !loc.is_empty() {
                format!("{}  ·  {}", ev.title, loc)
            } else {
                ev.title.clone()
            }
        } else {
            ev.title.clone()
        };

        canvas.draw_text_fit(&display_title, title_x, y0 + text_y_off, body_size, true, theme.text, TextAlign::Left, title_max_w);

        y0 += row_h;
    }
}
