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

    // Card Header Title
    let header_title = title.unwrap_or("Termine").to_uppercase();
    canvas.draw_text_fit(&header_title, x0, y0, type_scale::LABEL, true, theme.text_muted, TextAlign::Left, max_w);
    y0 += type_scale::LABEL + 16.0;

    // Divider line
    canvas.fill_rect(x0, y0, max_w, 1.5, theme.border);
    y0 += 14.0;

    if events.is_empty() {
        let msg = "Keine anstehenden Termine";
        canvas.draw_text(msg, x0 + max_w / 2.0, y0 + 20.0, type_scale::BODY, false, theme.text_muted, TextAlign::Center);
        return;
    }

    let time_col_w = if rect.w >= 600.0 { 180.0 } else { 140.0 };
    let row_h = 48.0;

    for ev in events {
        if y0 + row_h > rect.y + rect.h - 10.0 {
            break;
        }

        // Event Time and Date Badge
        let time_badge = match &ev.date_label {
            Some(label) if !label.is_empty() => {
                if ev.is_all_day {
                    format!("{} · Ganztägig", label)
                } else {
                    format!("{} {}", label, ev.time)
                }
            }
            _ => {
                if ev.is_all_day {
                    "Ganztägig".to_string()
                } else {
                    ev.time.clone()
                }
            }
        };
        canvas.draw_text_fit(&time_badge, x0, y0 + 6.0, type_scale::LABEL, true, theme.accent.solid, TextAlign::Left, time_col_w - 12.0);

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

        canvas.draw_text_fit(&display_title, title_x, y0 + 6.0, type_scale::BODY, true, theme.text, TextAlign::Left, title_max_w);

        y0 += row_h;
    }
}
