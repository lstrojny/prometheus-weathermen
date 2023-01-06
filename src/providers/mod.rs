mod cache;
mod meteoblue;
pub mod open_weather;
mod units;

use crate::providers::meteoblue::Meteoblue;
use crate::providers::open_weather::OpenWeather;
use crate::providers::units::Celsius;
use moka::sync::Cache;
use serde::Deserialize;
use std::fmt::{Display, Formatter};
use std::sync::Arc;
use std::time::Duration;
use std::vec::IntoIter;

#[derive(Deserialize, Debug, Clone)]
pub struct Providers {
    open_weather: Option<OpenWeather>,
    meteoblue: Option<Meteoblue>,
}

impl IntoIterator for Providers {
    type Item = Arc<dyn WeatherProvider + Send + Sync>;
    type IntoIter = IntoIter<Arc<dyn WeatherProvider + Send + Sync>>;

    fn into_iter(self) -> Self::IntoIter {
        let mut vec: Vec<Arc<dyn WeatherProvider + Send + Sync>> = vec![];

        if self.open_weather.is_some() {
            vec.push(Arc::new(self.open_weather.unwrap()));
        }

        if self.meteoblue.is_some() {
            vec.push(Arc::new(self.meteoblue.unwrap()));
        }

        IntoIter::into_iter(vec.into_iter())
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct Coordinate(f32);
impl Display for Coordinate {
    // Standardize 7 digits for coordinates and that should be plenty
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.7}", self.0)
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct Coordinates {
    #[serde(alias = "lat")]
    latitude: Coordinate,
    #[serde(alias = "lon")]
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

#[derive(Debug, Clone)]
pub struct WeatherRequest<T> {
    pub name: String,
    pub query: T,
}

pub trait WeatherProvider: std::fmt::Debug {
    fn for_coordinates(
        &self,
        cache: &Cache<String, String>,
        request: &WeatherRequest<Coordinates>,
    ) -> Result<Weather, String>;

    fn cache_lifetime(&self) -> Duration;
}
