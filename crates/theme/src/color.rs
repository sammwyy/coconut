use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{fmt, str::FromStr};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }
    pub fn mix(self, other: Self, amount: f32) -> Self {
        let t = amount.clamp(0., 1.);
        let c = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
        Self {
            r: c(self.r, other.r),
            g: c(self.g, other.g),
            b: c(self.b, other.b),
            a: c(self.a, other.a),
        }
    }
    pub fn opaque_on(self, background: Self) -> Self {
        let a = f32::from(self.a) / 255.;
        let mut result = background.mix(self, a);
        result.a = 255;
        result
    }
    pub fn luminance(self) -> f64 {
        let linear = |v: u8| {
            let v = f64::from(v) / 255.;
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(self.r) + 0.7152 * linear(self.g) + 0.0722 * linear(self.b)
    }
    pub fn foreground(self) -> Self {
        let l = self.luminance();
        if (l + 0.05) / 0.05 >= 1.05 / (l + 0.05) {
            Self::rgb(0, 0, 0)
        } else {
            Self::rgb(255, 255, 255)
        }
    }
}
impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.a == 255 {
            write!(f, "#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
        } else {
            write!(
                f,
                "#{:02x}{:02x}{:02x}{:02x}",
                self.r, self.g, self.b, self.a
            )
        }
    }
}
impl FromStr for Color {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, String> {
        let hex = value.strip_prefix('#').unwrap_or("");
        if !hex.is_ascii() || ![3, 4, 6, 8].contains(&hex.len()) {
            return Err(format!(
                "invalid color {value}; expected #RGB, #RGBA, #RRGGBB or #RRGGBBAA"
            ));
        }
        let expanded = if hex.len() <= 4 {
            hex.chars().flat_map(|c| [c, c]).collect::<String>()
        } else {
            hex.into()
        };
        let component = |i| {
            u8::from_str_radix(&expanded[i..i + 2], 16)
                .map_err(|_| format!("invalid color {value}"))
        };
        Ok(Self {
            r: component(0)?,
            g: component(2)?,
            b: component(4)?,
            a: if expanded.len() == 8 {
                component(6)?
            } else {
                255
            },
        })
    }
}
impl Serialize for Color {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}
impl<'de> Deserialize<'de> for Color {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        String::deserialize(d)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}
