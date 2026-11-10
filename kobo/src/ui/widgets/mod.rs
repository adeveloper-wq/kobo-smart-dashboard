pub mod calendar_card;
pub mod greeting_card;
pub mod header;
pub mod humidity_card;
pub mod weather_card;

pub use calendar_card::render_calendar_card;
pub use greeting_card::render_greeting_card;
pub use header::render_header;
pub use humidity_card::render_humidity_card;
pub use weather_card::render_weather_card;

use crate::display::{Canvas, TextAlign};
use crate::ui::layout::Rect;
use crate::ui::theme::{Theme, Tone};

/// Shared type scale (px at 300 ppi, enlarged for high legibility on small ~10 cm displays)
pub mod type_scale {
    pub const LABEL: f32 = 26.0;
    pub const BODY: f32 = 30.0;
    pub const TITLE: f32 = 38.0;
    pub const VALUE: f32 = 54.0;
    pub const HERO: f32 = 96.0;
}

pub const PAD: f32 = 24.0;

/// Card surface.
pub fn draw_card(canvas: &mut Canvas, theme: &Theme, rect: Rect, active: Option<Tone>) {
    let r = theme.card_radius;
    match active {
        Some(tone) => {
            canvas.fill_rounded_rect(rect.x, rect.y, rect.w, rect.h, r, tone.tint);
            canvas.stroke_rounded_rect(rect.x + 1.5, rect.y + 1.5, rect.w - 3.0, rect.h - 3.0, r - 1.5, 3.0, tone.solid);
        }
        None => {
            canvas.fill_rounded_rect(rect.x, rect.y, rect.w, rect.h, r, theme.surface);
            canvas.stroke_rounded_rect(rect.x + 1.0, rect.y + 1.0, rect.w - 2.0, rect.h - 2.0, r - 1.0, 2.0, theme.border);
        }
    }
}

/// Round icon badge; filled with the tone when active, neutral otherwise.
pub fn draw_icon_badge(canvas: &mut Canvas, theme: &Theme, x: f32, y: f32, d: f32, icon: &str, tone: Tone, active: bool) {
    let (bg, fg) = if active { (tone.solid, tone.on_solid) } else { (theme.surface_alt, tone.solid) };
    canvas.fill_circle(x + d / 2.0, y + d / 2.0, d / 2.0, bg);
    if !active {
        canvas.stroke_circle(x + d / 2.0, y + d / 2.0, d / 2.0 - 1.0, 2.0, theme.border);
    }
    let icon_size = d * 0.52;
    canvas.draw_icon(icon, x + (d - icon_size) / 2.0, y + (d - icon_size) / 2.0, icon_size, fg, 2.2);
}

/// Value in big type followed by a smaller unit; returns total width.
pub fn draw_value_unit(
    canvas: &mut Canvas,
    theme: &Theme,
    x: f32,
    y: f32,
    value: &str,
    unit: &str,
    size: f32,
    max_w: f32,
) -> f32 {
    let unit_size = (size * 0.5).max(20.0);
    let unit_w = if unit.is_empty() { 0.0 } else { canvas.measure_text(unit, unit_size, false).0 + 6.0 };
    let vw = canvas.draw_text_fit(value, x, y, size, true, theme.text, TextAlign::Left, (max_w - unit_w).max(10.0));
    if !unit.is_empty() {
        canvas.draw_text(unit, x + vw + 6.0, y + size * 0.85 - unit_size * 0.85, unit_size, false, theme.text_muted, TextAlign::Left);
    }
    vw + unit_w
}
