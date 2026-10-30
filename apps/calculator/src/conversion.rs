use std::f64::consts::{PI, TAU};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Category {
    Length,
    Area,
    Volume,
    Mass,
    Temperature,
    Energy,
    Speed,
    Time,
    Power,
    Pressure,
    Data,
    Angle,
}
impl Category {
    pub const ALL: [Self; 12] = [
        Self::Length,
        Self::Area,
        Self::Volume,
        Self::Mass,
        Self::Temperature,
        Self::Energy,
        Self::Speed,
        Self::Time,
        Self::Power,
        Self::Pressure,
        Self::Data,
        Self::Angle,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Length => "Length",
            Self::Area => "Area",
            Self::Volume => "Volume",
            Self::Mass => "Weight and mass",
            Self::Temperature => "Temperature",
            Self::Energy => "Energy",
            Self::Speed => "Speed",
            Self::Time => "Time",
            Self::Power => "Power",
            Self::Pressure => "Pressure",
            Self::Data => "Data",
            Self::Angle => "Angle",
        }
    }
    pub fn slug(self) -> &'static str {
        match self {
            Self::Length => "length",
            Self::Area => "area",
            Self::Volume => "volume",
            Self::Mass => "mass",
            Self::Temperature => "temperature",
            Self::Energy => "energy",
            Self::Speed => "speed",
            Self::Time => "time",
            Self::Power => "power",
            Self::Pressure => "pressure",
            Self::Data => "data",
            Self::Angle => "angle",
        }
    }
    pub fn units(self) -> &'static [Unit] {
        match self {
            Self::Length => {
                const {
                    &[
                        unit("Meters", "m", 1.0),
                        unit("Kilometers", "km", 1000.0),
                        unit("Centimeters", "cm", 0.01),
                        unit("Millimeters", "mm", 0.001),
                        unit("Micrometers", "µm", 1e-6),
                        unit("Nanometers", "nm", 1e-9),
                        unit("Inches", "in", 0.0254),
                        unit("Feet", "ft", 0.3048),
                        unit("Yards", "yd", 0.9144),
                        unit("Miles", "mi", 1609.344),
                        unit("Nautical miles", "nmi", 1852.0),
                    ]
                }
            }
            Self::Area => {
                const {
                    &[
                        unit("Square meters", "m²", 1.0),
                        unit("Square kilometers", "km²", 1e6),
                        unit("Square centimeters", "cm²", 1e-4),
                        unit("Square millimeters", "mm²", 1e-6),
                        unit("Hectares", "ha", 1e4),
                        unit("Acres", "ac", 4046.8564224),
                        unit("Square feet", "ft²", 0.09290304),
                        unit("Square inches", "in²", 0.00064516),
                        unit("Square yards", "yd²", 0.83612736),
                        unit("Square miles", "mi²", 2589988.110336),
                    ]
                }
            }
            Self::Volume => {
                const {
                    &[
                        unit("Liters", "L", 1e-3),
                        unit("Milliliters", "mL", 1e-6),
                        unit("Cubic meters", "m³", 1.0),
                        unit("Cubic centimeters", "cm³", 1e-6),
                        unit("US gallons", "US gal", 0.003785411784),
                        unit("Imperial gallons", "imp gal", 0.00454609),
                        unit("US quarts", "US qt", 0.000946352946),
                        unit("US pints", "US pt", 0.000473176473),
                        unit("US cups", "US cup", 0.0002365882365),
                        unit("US fluid ounces", "US fl oz", 0.0000295735295625),
                        unit("Cubic feet", "ft³", 0.028316846592),
                        unit("Cubic inches", "in³", 0.000016387064),
                    ]
                }
            }
            Self::Mass => {
                const {
                    &[
                        unit("Kilograms", "kg", 1.0),
                        unit("Grams", "g", 0.001),
                        unit("Milligrams", "mg", 1e-6),
                        unit("Micrograms", "µg", 1e-9),
                        unit("Metric tons", "t", 1000.0),
                        unit("Pounds", "lb", 0.45359237),
                        unit("Ounces", "oz", 0.028349523125),
                        unit("Stones", "st", 6.35029318),
                        unit("US short tons", "US ton", 907.18474),
                        unit("Imperial long tons", "imp ton", 1016.0469088),
                    ]
                }
            }
            Self::Temperature => {
                const {
                    &[
                        Unit {
                            name: "Celsius",
                            symbol: "°C",
                            factor: 1.0,
                            offset: 273.15,
                        },
                        Unit {
                            name: "Fahrenheit",
                            symbol: "°F",
                            factor: 5.0 / 9.0,
                            offset: 273.15 - 32.0 * 5.0 / 9.0,
                        },
                        unit("Kelvin", "K", 1.0),
                    ]
                }
            }
            Self::Energy => {
                const {
                    &[
                        unit("Joules", "J", 1.0),
                        unit("Kilojoules", "kJ", 1000.0),
                        unit("Megajoules", "MJ", 1e6),
                        unit("Calories (thermochemical)", "cal", 4.184),
                        unit("Kilocalories", "kcal", 4184.0),
                        unit("Watt-hours", "Wh", 3600.0),
                        unit("Kilowatt-hours", "kWh", 3.6e6),
                        unit("Electronvolts", "eV", 1.602176634e-19),
                    ]
                }
            }
            Self::Speed => {
                const {
                    &[
                        unit("Meters per second", "m/s", 1.0),
                        unit("Kilometers per hour", "km/h", 1.0 / 3.6),
                        unit("Miles per hour", "mph", 0.44704),
                        unit("Feet per second", "ft/s", 0.3048),
                        unit("Knots", "kn", 1852.0 / 3600.0),
                    ]
                }
            }
            Self::Time => {
                const {
                    &[
                        unit("Seconds", "s", 1.0),
                        unit("Milliseconds", "ms", 0.001),
                        unit("Microseconds", "µs", 1e-6),
                        unit("Nanoseconds", "ns", 1e-9),
                        unit("Minutes", "min", 60.0),
                        unit("Hours", "h", 3600.0),
                        unit("Days", "d", 86400.0),
                        unit("Weeks", "wk", 604800.0),
                    ]
                }
            }
            Self::Power => {
                const {
                    &[
                        unit("Watts", "W", 1.0),
                        unit("Kilowatts", "kW", 1000.0),
                        unit("Megawatts", "MW", 1e6),
                        unit("Milliwatts", "mW", 0.001),
                    ]
                }
            }
            Self::Pressure => {
                const {
                    &[
                        unit("Pascals", "Pa", 1.0),
                        unit("Kilopascals", "kPa", 1000.0),
                        unit("Megapascals", "MPa", 1e6),
                        unit("Bars", "bar", 1e5),
                        unit("Millibars", "mbar", 100.0),
                        unit("Atmospheres", "atm", 101325.0),
                        unit(
                            "Pounds per square inch",
                            "psi",
                            0.45359237 * 9.80665 / (0.0254 * 0.0254),
                        ),
                    ]
                }
            }
            Self::Data => {
                const {
                    &[
                        unit("Bytes", "B", 8.0),
                        unit("Bits", "bit", 1.0),
                        unit("Kilobytes (decimal)", "kB", 8000.0),
                        unit("Megabytes (decimal)", "MB", 8e6),
                        unit("Gigabytes (decimal)", "GB", 8e9),
                        unit("Terabytes (decimal)", "TB", 8e12),
                        unit("Kibibytes (binary)", "KiB", 8192.0),
                        unit("Mebibytes (binary)", "MiB", 8388608.0),
                        unit("Gibibytes (binary)", "GiB", 8589934592.0),
                        unit("Tebibytes (binary)", "TiB", 8796093022208.0),
                    ]
                }
            }
            Self::Angle => {
                const {
                    &[
                        unit("Degrees", "°", PI / 180.0),
                        unit("Radians", "rad", 1.0),
                        unit("Gradians", "gon", PI / 200.0),
                        unit("Revolutions", "rev", TAU),
                        unit("Arcminutes", "arcmin", PI / 10800.0),
                        unit("Arcseconds", "arcsec", PI / 648000.0),
                    ]
                }
            }
        }
    }
    pub fn defaults(self) -> (usize, usize) {
        match self {
            Self::Temperature => (0, 1),
            Self::Length => (0, 7),
            Self::Mass => (0, 5),
            Self::Volume => (0, 4),
            Self::Data => (0, 6),
            _ => (0, 1),
        }
    }
}
#[derive(Clone, Copy)]
pub struct Unit {
    pub name: &'static str,
    pub symbol: &'static str,
    pub factor: f64,
    pub offset: f64,
}
const fn unit(name: &'static str, symbol: &'static str, factor: f64) -> Unit {
    Unit {
        name,
        symbol,
        factor,
        offset: 0.0,
    }
}

pub fn convert(value: f64, category: Category, from: usize, to: usize) -> Result<f64, String> {
    if !value.is_finite() {
        return Err("Enter a finite number".into());
    }
    let units = category.units();
    let from = units.get(from).ok_or("Unknown source unit")?;
    let to = units.get(to).ok_or("Unknown destination unit")?;
    let base = value * from.factor + from.offset;
    if category == Category::Temperature && base < -1e-10 {
        return Err("Temperature cannot be below absolute zero".into());
    }
    let result = (base - to.offset) / to.factor;
    if !base.is_finite() || !result.is_finite() {
        return Err("Converted value is outside the finite number range".into());
    }
    Ok(if result == 0.0 { 0.0 } else { result })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn check(value: f64, category: Category, from: usize, to: usize, expected: f64) {
        let actual = convert(value, category, from, to).unwrap();
        assert!(
            (actual - expected).abs() < 1e-10 * expected.abs().max(1.0),
            "{actual} != {expected}"
        );
    }
    #[test]
    fn physical_units_and_offsets() {
        check(1.0, Category::Length, 9, 0, 1609.344);
        check(1.0, Category::Volume, 4, 0, 3.785411784);
        check(1.0, Category::Mass, 5, 0, 0.45359237);
        check(1.0, Category::Area, 5, 0, 4046.8564224);
        check(0.0, Category::Temperature, 0, 1, 32.0);
        check(212.0, Category::Temperature, 1, 0, 100.0);
        check(-40.0, Category::Temperature, 0, 1, -40.0);
        check(1.0, Category::Energy, 6, 0, 3600000.0);
        check(36.0, Category::Speed, 1, 0, 10.0);
        check(1.0, Category::Time, 5, 0, 3600.0);
        check(1.0, Category::Data, 6, 0, 1024.0);
        check(180.0, Category::Angle, 0, 1, PI);
    }
    #[test]
    fn all_units_round_trip_and_defaults_are_valid() {
        for category in Category::ALL {
            let (from, to) = category.defaults();
            assert!(from < category.units().len() && to < category.units().len());
            for from in 0..category.units().len() {
                for to in 0..category.units().len() {
                    let value = convert(1234.5, category, from, to).unwrap();
                    let restored = convert(value, category, to, from).unwrap();
                    assert!(
                        (restored - 1234.5).abs() < 1e-7,
                        "{} {from} {to}",
                        category.name()
                    );
                }
            }
        }
        assert!(convert(-1.0, Category::Temperature, 2, 0).is_err());
        assert!(convert(f64::MAX, Category::Length, 1, 0).is_err());
        assert!(convert(f64::NAN, Category::Mass, 0, 1).is_err());
        assert!(convert(1.0, Category::Area, usize::MAX, 0).is_err());
    }
}
