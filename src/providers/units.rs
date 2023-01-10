use serde::Deserialize;
use std::fmt::Debug;

#[derive(Deserialize, Debug)]
pub struct Kelvin(f32);

impl From<f32> for Kelvin {
    fn from(value: f32) -> Self {
        Self(value)
    }
}

const ABSOLUTE_ZERO_IN_CELSIUS: f32 = 273.15;
impl ToCelsius for Kelvin {
    fn to_celsius(&self) -> Celsius {
        Celsius(self.0 - ABSOLUTE_ZERO_IN_CELSIUS)
    }
}

#[derive(Deserialize, Debug)]
pub struct Celsius(f32);

impl From<f32> for Celsius {
    fn from(value: f32) -> Self {
        Self(value)
    }
}

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

#[derive(Deserialize, Debug)]
pub struct Fahrenheit(f32);

pub trait ToCelsius {
    fn to_celsius(&self) -> Celsius;
}

impl From<f32> for Fahrenheit {
    fn from(value: f32) -> Self {
        Self(value)
    }
}

impl ToCelsius for Fahrenheit {
    fn to_celsius(&self) -> Celsius {
        Celsius(((self.0 - 32_f32) * 5_f32) / 9_f32)
    }
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum Ratio {
    Percentage(u16),
    Ratio(f64),
}

impl Ratio {
    pub fn as_f64(&self) -> f64 {
        match self {
            Self::Ratio(v) => *v,
            Self::Percentage(v) => f64::from(*v) / 100.0,
        }
    }
}
