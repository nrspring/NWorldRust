//! Colors for things in the game, such as countries: red, green and blue, a byte each. In the saved
//! files a color is its hex code, like `"#4f8cff"`.

use macroquad::color::Color;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::ids;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    /// The color with this `hue` (in degrees, going round from red through green and blue back to red),
    /// `saturation` (0 grey to 1 full) and `value` (0 black to 1 bright).
    pub fn from_hsv(hue: f32, saturation: f32, value: f32) -> Self {
        let (s, v) = (saturation.clamp(0.0, 1.0), value.clamp(0.0, 1.0));
        let h = hue.rem_euclid(360.0) / 60.0;
        let chroma = v * s;
        let x = chroma * (1.0 - (h % 2.0 - 1.0).abs());
        let (r, g, b) = match h as u32 {
            0 => (chroma, x, 0.0),
            1 => (x, chroma, 0.0),
            2 => (0.0, chroma, x),
            3 => (0.0, x, chroma),
            4 => (x, 0.0, chroma),
            _ => (chroma, 0.0, x),
        };
        let m = v - chroma;
        let byte = |c: f32| ((c + m) * 255.0).round() as u8;
        Self { r: byte(r), g: byte(g), b: byte(b) }
    }

    /// Its hue in degrees, saturation and value, as `from_hsv` takes them. A grey has hue 0.
    pub fn to_hsv(self) -> (f32, f32, f32) {
        let (r, g, b) = (self.r as f32 / 255.0, self.g as f32 / 255.0, self.b as f32 / 255.0);
        let max = r.max(g).max(b);
        let delta = max - r.min(g).min(b);
        let hue = if delta == 0.0 {
            0.0
        } else if max == r {
            60.0 * ((g - b) / delta).rem_euclid(6.0)
        } else if max == g {
            60.0 * ((b - r) / delta + 2.0)
        } else {
            60.0 * ((r - g) / delta + 4.0)
        };
        (hue, if max == 0.0 { 0.0 } else { delta / max }, max)
    }

    /// `#rrggbb`, in lower case.
    pub fn to_hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }

    /// The color of a code like `#4f8cff` (the `#` is needed, the capitals don't matter).
    pub fn from_hex(code: &str) -> Option<Self> {
        let digits = code.strip_prefix('#')?;
        if digits.len() != 6 || !digits.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        let byte = |at: usize| u8::from_str_radix(&digits[at..at + 2], 16).ok();
        Some(Self { r: byte(0)?, g: byte(2)?, b: byte(4)? })
    }

    /// The color to draw with.
    pub fn color(self) -> Color {
        Color::from_rgba(self.r, self.g, self.b, 255)
    }

    /// A random color, that is a pleasant one (not too pale, dark or dull) and as unlike `others` as
    /// the best of a handful of tries.
    pub fn random_distinct_from(others: &[Rgb]) -> Self {
        let unit = ids::random_unit;
        let mut best = (f32::NEG_INFINITY, Self { r: 0, g: 0, b: 0 });
        for _ in 0..16 {
            let candidate = Self::from_hsv(unit() * 360.0, 0.5 + unit() * 0.25, 0.75 + unit() * 0.2);
            let apart = others.iter().map(|o| candidate.distance(*o)).fold(f32::INFINITY, f32::min);
            if apart > best.0 {
                best = (apart, candidate);
            }
        }
        best.1
    }

    /// How different two colors look, roughly: the distance between them, with green counting most.
    fn distance(self, other: Rgb) -> f32 {
        let d = |a: u8, b: u8| a as f32 - b as f32;
        (2.0 * d(self.r, other.r).powi(2) + 4.0 * d(self.g, other.g).powi(2) + 3.0 * d(self.b, other.b).powi(2)).sqrt()
    }
}

impl Serialize for Rgb {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for Rgb {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let code = String::deserialize(deserializer)?;
        Rgb::from_hex(&code).ok_or_else(|| serde::de::Error::custom(format!("{code:?} is not a color like \"#4f8cff\"")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_main_colors_come_out_right() {
        assert_eq!(Rgb::from_hsv(0.0, 1.0, 1.0), Rgb { r: 255, g: 0, b: 0 });
        assert_eq!(Rgb::from_hsv(120.0, 1.0, 1.0), Rgb { r: 0, g: 255, b: 0 });
        assert_eq!(Rgb::from_hsv(240.0, 1.0, 1.0), Rgb { r: 0, g: 0, b: 255 });
        assert_eq!(Rgb::from_hsv(60.0, 1.0, 1.0), Rgb { r: 255, g: 255, b: 0 });
        assert_eq!(Rgb::from_hsv(77.0, 0.0, 1.0), Rgb { r: 255, g: 255, b: 255 });
        assert_eq!(Rgb::from_hsv(77.0, 0.5, 0.0), Rgb { r: 0, g: 0, b: 0 });
        // Going all the way round is back where it started.
        assert_eq!(Rgb::from_hsv(360.0, 1.0, 1.0), Rgb::from_hsv(0.0, 1.0, 1.0));
    }

    #[test]
    fn hsv_and_back_gives_the_same_color() {
        for (r, g, b) in [(79, 140, 255), (229, 72, 77), (0, 0, 0), (255, 255, 255), (128, 128, 128), (12, 200, 99)] {
            let c = Rgb { r, g, b };
            let (h, s, v) = c.to_hsv();
            assert_eq!(Rgb::from_hsv(h, s, v), c);
        }
    }

    #[test]
    fn a_color_is_a_hex_code() {
        let c = Rgb { r: 79, g: 140, b: 255 };
        assert_eq!(c.to_hex(), "#4f8cff");
        assert_eq!(Rgb::from_hex("#4f8cff"), Some(c));
        assert_eq!(Rgb::from_hex("#4F8CFF"), Some(c));
        for bad in ["4f8cff", "#4f8cf", "#4f8cfff", "#4f8cgg", "", "#"] {
            assert_eq!(Rgb::from_hex(bad), None, "{bad}");
        }
    }

    #[test]
    fn a_color_is_saved_as_its_hex_code() {
        let c = Rgb { r: 1, g: 2, b: 3 };
        let json = serde_json::to_string(&c).unwrap();
        assert_eq!(json, "\"#010203\"");
        assert_eq!(serde_json::from_str::<Rgb>(&json).unwrap(), c);
        assert!(serde_json::from_str::<Rgb>("\"red\"").is_err());
    }

    #[test]
    fn random_colors_are_not_dull_dark_or_pale() {
        for _ in 0..200 {
            let (_, s, v) = Rgb::random_distinct_from(&[]).to_hsv();
            assert!((0.45..=0.8).contains(&s) && (0.7..=1.0).contains(&v), "s {s} v {v}");
        }
    }

    #[test]
    fn a_new_color_keeps_away_from_the_ones_there_are() {
        // Against a country already red, the pick should come out as far from red as chance of 16 tries allows,
        // which is on average well away; so check it isn't the red itself every time.
        let red = Rgb { r: 229, g: 72, b: 77 };
        let near = (0..200).filter(|_| Rgb::random_distinct_from(&[red]).distance(red) < 60.0).count();
        assert!(near < 5, "{near} of 200 were close to red");
        // And they vary.
        let colors: std::collections::HashSet<String> = (0..50).map(|_| Rgb::random_distinct_from(&[red]).to_hex()).collect();
        assert!(colors.len() > 40);
    }
}
