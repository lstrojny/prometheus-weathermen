use derive_more::{Display, From, Into};
use rocket::serde::Serialize;
use serde::Deserialize;
use std::fmt::Debug;

#[derive(Deserialize, Debug, Copy, Clone, From, PartialEq)]
pub struct Kelvin(f32);

impl ToCelsius for Kelvin {
    fn to_celsius(&self) -> Celsius {
        Celsius(self.0 + CELSIUS_ABSOLUTE_ZERO)
    }
}

#[derive(Deserialize, Debug, Copy, Clone, From, Into, PartialEq)]
#[into(f64)]
pub struct Celsius(f32);

const CELSIUS_ABSOLUTE_ZERO: f32 = -273.15;

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

const FAHRENHEIT_FREEZING_POINT: f32 = 32.0;
const FAHRENHEIT_CELSIUS_RATIO: f32 = 5.0 / 9.0;

impl ToCelsius for Fahrenheit {
    fn to_celsius(&self) -> Celsius {
        Celsius((self.0 - FAHRENHEIT_FREEZING_POINT) * FAHRENHEIT_CELSIUS_RATIO)
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
#[display("{_0:.7}")]
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

#[cfg(test)]
mod test {
    use crate::providers::units::{Celsius, Fahrenheit, Kelvin, ToCelsius};

    #[test]
    fn test_fahrenheit_to_celsius() {
        assert_eq!(Fahrenheit(32_f32).to_celsius(), Celsius(0_f32));
        assert_eq!(Fahrenheit(100_f32).to_celsius(), Celsius(37.77778_f32));
        assert_eq!(Fahrenheit(212_f32).to_celsius(), Celsius(100.00001_f32));
    }

    #[test]
    fn test_kelvin_to_celsius() {
        assert_eq!(Kelvin(273.15_f32).to_celsius(), Celsius(0_f32));
        assert_eq!(Kelvin(373.15_f32).to_celsius(), Celsius(100_f32));
    }

    #[test]
    fn test_celsius_to_celsius() {
        assert_eq!(Celsius(37_f32).to_celsius(), Celsius(37_f32));
    }
}
