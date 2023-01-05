pub mod open_weather;
mod units;

use crate::provider::open_weather::OpenWeather;
use crate::provider::units::Celsius;
use serde::Deserialize;
use std::fmt::{Display, Formatter};
use std::sync::Arc;
use std::vec::IntoIter;

#[derive(Deserialize, Debug, Clone)]
pub struct Provider {
    open_weather: Option<OpenWeather>,
}

impl IntoIterator for Provider {
    type Item = Arc<dyn WeatherProvider + Send + Sync>;
    type IntoIter = IntoIter<Arc<dyn WeatherProvider + Send + Sync>>;

    fn into_iter(self) -> Self::IntoIter {
        let mut vec: Vec<Arc<dyn WeatherProvider + Send + Sync>> = vec![];
        if self.open_weather.is_some() {
            vec.push(Arc::new(self.open_weather.unwrap()));
        }

        IntoIter::into_iter(vec.into_iter())
    }
}

#[derive(Deserialize, Debug, Copy, Clone)]
pub struct Coordinate(f32);
impl Display for Coordinate {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.7}", self.0)
    }
}

#[derive(Deserialize, Debug, Copy, Clone)]
pub struct Coordinates {
    latitude: Coordinate,
    longitude: Coordinate,
}
impl Coordinates {
    pub fn new(latitude: Coordinate, longitude: Coordinate) -> Self {
        Coordinates {
            latitude,
            longitude,
        }
    }
    pub fn get_latitude(&self) -> Coordinate {
        self.latitude
    }
    pub fn get_longitude(&self) -> Coordinate {
        self.longitude
    }
}

#[derive(Debug)]
pub struct Weather {
    pub city: String,
    pub temperature: Celsius,
    pub coordinates: Coordinates,
}

pub trait WeatherProvider {
    fn for_coordinates(&self, coordinates: Coordinates) -> Result<Weather, String>;
}
