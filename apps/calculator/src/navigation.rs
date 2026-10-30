use crate::conversion::Category;

pub const DRAWER_WIDTH: u32 = 200;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Mode {
    #[default]
    Standard,
    Scientific,
    Converter(Category),
}
impl Mode {
    pub fn name(self) -> &'static str {
        match self {
            Self::Standard => "Standard",
            Self::Scientific => "Scientific",
            Self::Converter(category) => category.name(),
        }
    }
    pub fn path(self) -> String {
        match self {
            Self::Standard => "/calculator/standard".into(),
            Self::Scientific => "/calculator/scientific".into(),
            Self::Converter(category) => format!("/converter/{}", category.slug()),
        }
    }
    pub fn from_path(path: &str) -> Self {
        if path == "/calculator/scientific" {
            Self::Scientific
        } else {
            Category::ALL
                .into_iter()
                .find(|category| path == format!("/converter/{}", category.slug()))
                .map_or(Self::Standard, Self::Converter)
        }
    }
    pub fn sizing(self) -> Sizing {
        let (default, minimum) = match self {
            Self::Standard => ((360, 520), (320, 480)),
            Self::Scientific => ((720, 560), (620, 500)),
            Self::Converter(_) => ((500, 620), (460, 540)),
        };
        Sizing { default, minimum }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sizing {
    pub default: (u32, u32),
    pub minimum: (u32, u32),
}
impl Sizing {
    pub fn transition(self, next: Self, current: (u32, u32)) -> (u32, u32) {
        if current.0.abs_diff(self.default.0) <= 1 && current.1.abs_diff(self.default.1) <= 1 {
            next.default
        } else {
            (current.0.max(next.minimum.0), current.1.max(next.minimum.1))
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_resize_in_both_directions_custom_sizes_are_preserved_or_clamped() {
        let normal = Mode::Standard.sizing();
        let scientific = Mode::Scientific.sizing();
        assert_eq!(
            normal.transition(scientific, normal.default),
            scientific.default
        );
        assert_eq!(
            scientific.transition(normal, scientific.default),
            normal.default
        );
        assert_eq!(normal.transition(scientific, (1200, 900)), (1200, 900));
        assert_eq!(normal.transition(scientific, (700, 700)), (700, 700));
        assert_eq!(
            normal.transition(scientific, (340, 490)),
            scientific.minimum
        );
        let converter = Mode::Converter(Category::Length).sizing();
        assert_eq!(
            converter.transition(normal, converter.default),
            normal.default
        );
        assert_eq!(scientific.transition(normal, (800, 700)), (800, 700));
        assert_ne!(normal.minimum, scientific.minimum);
    }
}
