use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct Kelvin(f32);

#[derive(Deserialize, Debug)]
pub struct Celsius(f32);

impl Celsius {
    pub(crate) fn to_f32(&self) -> f32 {
        self.0
    }
}

#[derive(Deserialize, Debug)]
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

impl ToCelsius for Fahrenheit {
    fn to_celsius(&self) -> Celsius {
        Celsius(((self.0 - 32_f32) * 5_f32) / 9_f32)
    }
}

impl ToCelsius for Celsius {
    fn to_celsius(&self) -> Celsius {
        Celsius(self.0)
    }
}
