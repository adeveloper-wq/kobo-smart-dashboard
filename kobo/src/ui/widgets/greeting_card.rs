use crate::display::{Canvas, TextAlign};
use crate::ui::layout::{InteractiveTarget, Rect};
use crate::ui::theme::Theme;
use crate::ui::widgets::{draw_card, PAD};

/// One word of the flowing greeting, or the weather chip that closes it.
enum Item {
    Word(String),
    Weather,
}

/// Greeting text with the current weather (icon + temperature) directly after
/// it. Both wrap together; the chip never splits. The date is left out: the
/// header has it.
pub fn render_greeting_card(
    canvas: &mut Canvas,
    theme: &Theme,
    rect: Rect,
    title: Option<&str>,
    temperature: Option<f32>,
    condition: Option<&str>,
    humidity: Option<f32>,
    _targets: &mut Vec<InteractiveTarget>,
) {
    draw_card(canvas, theme, rect, None);

    let x0 = rect.x + PAD;
    let max_w = rect.w - 2.0 * PAD;
    let inner_h = rect.h - 24.0;
    let cond = condition.unwrap_or("unavailable");
    let has_weather = temperature.is_some() || condition.is_some() || humidity.is_some();
    let temp_part = temperature.map(|t| format!("{:.0}°", t)).unwrap_or_else(|| "--".to_string());
    let temp_str = match humidity {
        Some(h) => format!("{} · {:.0}%", temp_part, h),
        None => temp_part,
    };

    let mut items: Vec<Item> = title.unwrap_or("Willkommen").split_whitespace().map(|w| Item::Word(w.to_string())).collect();
    if has_weather {
        items.push(Item::Weather);
    }

    // Largest size at which everything fits the card height
    let mut size = 46.0_f32;
    let (lines, line_h, chip_art) = loop {
        let line_h = size * 1.15;
        let chip_art = size * 0.9;
        let space = canvas.measure_text(" ", size, true).0.max(size * 0.28);
        let mut lines: Vec<Vec<(usize, f32)>> = vec![Vec::new()];
        let mut x = 0.0_f32;
        for (i, it) in items.iter().enumerate() {
            let w = match it {
                Item::Word(s) => canvas.measure_text(s, size, true).0,
                Item::Weather => chip_art + 8.0 + canvas.measure_text(&temp_str, size, true).0,
            };
            if x > 0.0 && x + space + w > max_w {
                lines.push(Vec::new());
                x = 0.0;
            }
            if x > 0.0 {
                x += space;
            }
            lines.last_mut().unwrap().push((i, x));
            x += w;
        }
        if lines.len() as f32 * line_h <= inner_h || size <= 28.0 {
            break (lines, line_h, chip_art);
        }
        size -= 4.0;
    };

    let total_h = lines.len() as f32 * line_h;
    let mut y = rect.y + (rect.h - total_h) / 2.0;
    for line in &lines {
        for &(i, lx) in line {
            let x = x0 + lx;
            match &items[i] {
                Item::Word(s) => {
                    canvas.draw_text(s, x, y - size * 0.08, size, true, theme.text, TextAlign::Left);
                }
                Item::Weather => {
                    canvas.draw_weather_art(cond, x, y + (line_h - chip_art) / 2.0 - 2.0, chip_art, theme.weather);
                    canvas.draw_text(&temp_str, x + chip_art + 8.0, y - size * 0.08, size, true, theme.text, TextAlign::Left);
                }
            }
        }
        y += line_h;
    }
}
