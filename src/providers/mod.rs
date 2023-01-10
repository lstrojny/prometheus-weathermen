mod cache;
mod meteoblue;
mod open_weather;
pub mod units;

use crate::providers::meteoblue::Meteoblue;
use crate::providers::open_weather::OpenWeather;
use crate::providers::units::{Celsius, Ratio};
use moka::sync::Cache;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::sync::Arc;
use std::time::Duration;
use std::vec::IntoIter;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Providers {
    open_weather: Option<OpenWeather>,
    meteoblue: Option<Meteoblue>,
}

impl IntoIterator for Providers {
    type Item = Arc<dyn WeatherProvider + Send + Sync>;
    type IntoIter = IntoIter<Arc<dyn WeatherProvider + Send + Sync>>;

    fn into_iter(self) -> Self::IntoIter {
        let mut vec: Vec<Arc<dyn WeatherProvider + Send + Sync>> = vec![];

        if let Some(provider) = self.open_weather {
            vec.push(Arc::new(provider));
        }

        if let Some(provider) = self.meteoblue {
            vec.push(Arc::new(provider));
        }

        IntoIter::into_iter(vec.into_iter())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Coordinate(f32);
impl Display for Coordinate {
    // Standardize 7 digits for coordinates and that should be plenty
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.7}", self.0)
    }
}

impl From<f32> for Coordinate {
    fn from(value: f32) -> Self {
        Self(value)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Coordinates {
    #[serde(alias = "lat")]
    pub latitude: Coordinate,
    #[serde(alias = "lon")]
    pub longitude: Coordinate,
}

#[derive(Debug)]
pub struct Weather {
    pub location: String,
    pub source: String,
    pub city: String,
    pub temperature: Celsius,
    pub relative_humidity: Option<Ratio>,
    pub coordinates: Coordinates,
}

#[derive(Debug, Clone)]
pub struct WeatherRequest<T> {
    pub name: String,
    pub query: T,
}

pub trait WeatherProvider: std::fmt::Debug {
    fn id(&self) -> &str;

    fn for_coordinates(
        &self,
        cache: &Cache<String, String>,
        request: &WeatherRequest<Coordinates>,
    ) -> anyhow::Result<Weather>;

    fn refresh_interval(&self) -> Duration;
}
