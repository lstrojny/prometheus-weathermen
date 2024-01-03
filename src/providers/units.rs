use derive_more::{Display, From, Into};
use rocket::serde::Serialize;
use serde::Deserialize;
use std::fmt::Debug;

#[derive(Deserialize, Debug, Copy, Clone, From, PartialEq)]
pub struct Kelvin(f32);

const ABSOLUTE_ZERO_IN_CELSIUS: f32 = 273.15;
impl ToCelsius for Kelvin {
    fn to_celsius(&self) -> Celsius {
        Celsius(self.0 - ABSOLUTE_ZERO_IN_CELSIUS)
    }
}

#[derive(Deserialize, Debug, Copy, Clone, From, Into, PartialEq)]
#[into(types(f64))]
pub struct Celsius(f32);

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
    Percentage(f64),
    Fraction(f64),
}

impl From<Ratio> for f64 {
    fn from(value: Ratio) -> Self {
        match value {
            Ratio::Fraction(v) => v,
            Ratio::Percentage(v) => v / 100.0_f64,
        }
    }
}

#[derive(Serialize, Deserialize, From, Into, Debug, Clone, Display)]
#[display(fmt = "{_0:.7}")]
pub struct Coordinate(f64);

impl PartialEq for Coordinate {
    fn eq(&self, other: &Self) -> bool {
        (self.0 - other.0).abs() < 0.000_000_1_f64
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
