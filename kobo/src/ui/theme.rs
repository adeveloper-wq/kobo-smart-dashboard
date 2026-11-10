use crate::config::ThemeConfig;
use crate::display::color::Color;
use crate::display::icons::WeatherInk;

/// A semantic tone: `solid` for icons/fills, `tint` for large active
/// surfaces, `on_solid` for content drawn on top of `solid`.
#[derive(Debug, Clone, Copy)]
pub struct Tone {
    pub solid: Color,
    pub tint: Color,
    pub on_solid: Color,
}

impl Tone {
    const fn new(solid: Color, tint: Color, on_solid: Color) -> Self {
        Self { solid, tint, on_solid }
    }
}

#[derive(Debug, Clone)]
pub struct Theme {
    pub bg: Color,
    pub surface: Color,
    pub surface_alt: Color,
    pub border: Color,
    pub border_strong: Color,
    pub text: Color,
    pub text_muted: Color,
    pub text_faint: Color,
    pub card_radius: f32,
    pub is_dark: bool,

    pub accent: Tone,
    pub warm: Tone,
    pub heat: Tone,
    pub cool: Tone,
    pub eco: Tone,
    pub media: Tone,
    pub idle: Tone,

    pub weather: WeatherInk,
}

const WHITE: Color = Color::rgb(255, 255, 255);

impl Theme {
    /// High-contrast black and white is the only supported mode.
    pub fn from_config(config: &ThemeConfig) -> Self {
        let radius = config.card_corner_radius;
        let ink = Color::rgb(0, 0, 0);
        let tone = Tone::new(ink, Color::rgb(222, 222, 222), WHITE);
        Self {
            bg: WHITE,
            surface: WHITE,
            surface_alt: Color::rgb(238, 238, 238),
            border: ink,
            border_strong: ink,
            text: ink,
            text_muted: Color::rgb(50, 50, 50),
            text_faint: Color::rgb(100, 100, 100),
            card_radius: radius,
            is_dark: false,
            accent: tone,
            warm: tone,
            heat: tone,
            cool: tone,
            eco: tone,
            media: tone,
            idle: Tone::new(Color::rgb(60, 60, 60), Color::rgb(238, 238, 238), WHITE),
            weather: WeatherInk {
                line: ink,
                sun: Color::rgb(160, 160, 160),
                cloud: WHITE,
                rain: ink,
            },
        }
    }
}
