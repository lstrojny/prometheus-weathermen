pub mod cache;
mod dwd;
mod meteoblue;
mod nogoodnik;
mod open_weather;
mod tomorrow;
pub mod units;

use crate::providers::cache::RequestBody;
use crate::providers::dwd::Dwd;
use crate::providers::meteoblue::Meteoblue;
use crate::providers::nogoodnik::Nogoodnik;
use crate::providers::open_weather::OpenWeather;
use crate::providers::tomorrow::Tomorrow;
use crate::providers::units::{Celsius, Ratio};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use std::vec::IntoIter;
use units::Coordinates;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Providers {
    open_weather: Option<OpenWeather>,
    meteoblue: Option<Meteoblue>,
    tomorrow: Option<Tomorrow>,
    dwd: Option<Dwd>,
    nogoodnik: Option<Nogoodnik>,
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

        if let Some(provider) = self.tomorrow {
            vec.push(Arc::new(provider));
        }

        if let Some(provider) = self.dwd {
            vec.push(Arc::new(provider));
        }

        if let Some(provider) = self.nogoodnik {
            vec.push(Arc::new(provider));
        }

        IntoIter::into_iter(vec.into_iter())
    }
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
        cache: &RequestBody,
        request: &WeatherRequest<Coordinates>,
    ) -> anyhow::Result<Weather>;

    fn refresh_interval(&self) -> Duration;
}
