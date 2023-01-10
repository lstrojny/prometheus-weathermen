use crate::providers::cache::{reqwest_cached_body_json, Configuration};
use crate::providers::units::{Coordinates, Kelvin, Ratio, ToCelsius};
use crate::providers::{Weather, WeatherProvider, WeatherRequest};
use moka::sync::Cache;
use reqwest::{Method, Url};
use rocket::serde::Deserialize;
use serde::Serialize;
use std::string::ToString;
use std::time::Duration;

const SOURCE_URI: &str = "org.openweathermap";
const ENDPOINT_URL: &str = "https://api.openweathermap.org/data/2.5/weather";

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OpenWeather {
    pub api_key: String,
    #[serde(flatten)]
    pub cache: Configuration,
}

#[derive(Deserialize, Debug)]
struct OpenWeatherResponseMain {
    temp: Kelvin,
    humidity: Ratio,
}

#[derive(Deserialize, Debug)]
struct OpenWeatherResponse {
    coord: Coordinates,
    name: String,
    main: OpenWeatherResponseMain,
}

impl WeatherProvider for OpenWeather {
    fn id(&self) -> &str {
        SOURCE_URI
    }

    fn for_coordinates(
        &self,
        cache: &Cache<String, String>,
        request: &WeatherRequest<Coordinates>,
    ) -> anyhow::Result<Weather> {
        let url = Url::parse_with_params(
            ENDPOINT_URL,
            &[
                ("lat", request.query.latitude.to_string()),
                ("lon", request.query.longitude.to_string()),
                ("appid", self.api_key.to_string()),
            ],
        )?;

        let client = reqwest::blocking::Client::new();

        let response = reqwest_cached_body_json::<OpenWeatherResponse>(
            SOURCE_URI,
            cache,
            &client,
            Method::GET,
            url,
        )?;

        Ok(Weather {
            source: SOURCE_URI.into(),
            location: request.name.clone(),
            city: response.name,
            temperature: response.main.temp.to_celsius(),
            relative_humidity: Some(response.main.humidity),
            coordinates: response.coord,
        })
    }

    fn refresh_interval(&self) -> Duration {
        self.cache.refresh_interval
    }
}
