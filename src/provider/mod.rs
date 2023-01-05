pub mod open_weather;
mod units;

use crate::provider::open_weather::OpenWeather;
use crate::provider::units::Celsius;
use serde::Deserialize;
use std::fmt::{Display, Formatter};
use std::sync::Arc;
use std::vec::IntoIter;

#[derive(Deserialize, Debug, Clone)]
pub struct Providers {
    open_weather: Option<OpenWeather>,
}

impl IntoIterator for Providers {
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

#[derive(Deserialize, Debug, Clone)]
pub struct Coordinate(f32);
impl Display for Coordinate {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.7}", self.0)
    }
}

#[derive(Deserialize, Debug, Clone)]
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
        self.latitude.clone()
    }
    pub fn get_longitude(&self) -> Coordinate {
        self.longitude.clone()
    }
}

#[derive(Debug)]
pub struct Weather {
    pub location: String,
    pub source: String,
    pub city: String,
    pub temperature: Celsius,
    pub coordinates: Coordinates,
}

#[derive(Debug)]
pub struct WeatherRequest {
    pub name: String,
    pub coordinates: Coordinates,
}

pub trait WeatherProvider {
    fn for_coordinates(&self, request: WeatherRequest) -> Result<Weather, String>;
}
