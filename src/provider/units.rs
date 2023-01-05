use serde::Deserialize;
use std::ops::Deref;

#[derive(Deserialize)]
pub struct Kelvin(f32);

#[derive(Deserialize)]
pub struct Celsius(f32);

#[derive(Deserialize)]
pub struct Fahrenheit(f32);

pub trait ToCelsius {
    fn to_celsius(&self) -> Celsius;
}

const ABSOLUTE_ZERO_IN_CELSIUS: f32 = 273.15;
impl ToCelsius for Kelvin {
    fn to_celsius(&self) -> Celsius {
        Celsius(self.0 - ABSOLUTE_ZERO_IN_CELSIUS)
    }
}

impl Deref for Kelvin {
    type Target = f32;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl ToCelsius for Fahrenheit {
    fn to_celsius(&self) -> Celsius {
        Celsius(((self.0 - 32_f32) * 5_f32) / 9_f32)
    }
}

impl Deref for Fahrenheit {
    type Target = f32;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl ToCelsius for Celsius {
    fn to_celsius(&self) -> Celsius {
        Celsius(self.0)
    }
}

impl Deref for Celsius {
    type Target = f32;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
