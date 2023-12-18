use derive_more::{From, Into};
use rocket::serde::Serialize;
use serde::Deserialize;
use std::fmt::{Debug, Display, Formatter};

#[derive(Deserialize, Debug, Copy, Clone, From, PartialEq)]
pub struct Kelvin(f32);

const ABSOLUTE_ZERO_IN_CELSIUS: f32 = 273.15;
impl ToCelsius for Kelvin {
    fn to_celsius(&self) -> Celsius {
        Celsius(self.0 - ABSOLUTE_ZERO_IN_CELSIUS)
    }
}

#[derive(Deserialize, Debug, Copy, Clone, From, Into, PartialEq)]
pub struct Celsius(f32);

impl From<Celsius> for f64 {
    fn from(value: Celsius) -> Self {
        Self::from(value.0)
    }
}

impl ToCelsius for Celsius {
    fn to_celsius(&self) -> Self {
        Self(self.0)
    }
}

#[derive(Deserialize, Debug, Copy, Clone, From, PartialEq)]
pub struct Fahrenheit(f32);

pub trait ToCelsius {
    fn to_celsius(&self) -> Celsius;
}

impl ToCelsius for Fahrenheit {
    fn to_celsius(&self) -> Celsius {
        Celsius(((self.0 - 32_f32) * 5_f32) / 9_f32)
    }
}

#[derive(Deserialize, Debug, Copy, Clone, PartialEq)]
#[serde(untagged)]
pub enum Ratio {
    Percentage(u16),
    PercentageDecimal(f64),
    Ratio(f64),
}

impl Ratio {
    pub fn as_f64(&self) -> f64 {
        match self {
            Self::Ratio(v) => *v,
            Self::Percentage(v) => f64::from(*v) / 100.0,
            Self::PercentageDecimal(v) => v / 100.0,
        }
    }
}

#[derive(Serialize, Deserialize, From, Into, Debug, Clone)]
pub struct Coordinate(f64);

impl PartialEq for Coordinate {
    fn eq(&self, other: &Self) -> bool {
        (self.0 - other.0).abs() < 0.000_000_1
    }
}

impl Display for Coordinate {
    // Standardize 7 digits for coordinates and that should be plenty
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.7}", self.0)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Coordinates {
    #[serde(alias = "lat")]
    pub latitude: Coordinate,
    #[serde(alias = "lon")]
    pub longitude: Coordinate,
}

#[derive(Debug, Clone, From, Into)]
pub struct Meters(f64);
