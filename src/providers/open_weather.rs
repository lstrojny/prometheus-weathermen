use crate::providers::cache::{reqwest_cached_body_json, CacheConfiguration};
use crate::providers::units::{Kelvin, ToCelsius};
use crate::providers::{Coordinates, Weather, WeatherProvider, WeatherRequest};
use moka::sync::Cache;
use reqwest::{Method, Url};
use rocket::serde::Deserialize;
use std::string::ToString;
use std::time::Duration;

const SOURCE_URI: &str = "org.openweathermap";
const ENDPOINT_URL: &str = "https://api.openweathermap.org/data/2.5/weather";

#[derive(Deserialize, Debug, Clone)]
pub struct OpenWeather {
    pub api_key: String,
    #[serde(flatten)]
    pub cache: CacheConfiguration,
}

#[derive(Deserialize)]
struct OpenWeatherResponseMain {
    temp: Kelvin,
}

#[derive(Deserialize)]
struct OpenWeatherResponse {
    coord: Coordinates,
    name: String,
    main: OpenWeatherResponseMain,
}

impl WeatherProvider for OpenWeather {
    fn for_coordinates(
        &self,
        cache: &Cache<String, String>,
        request: &WeatherRequest<Coordinates>,
    ) -> anyhow::Result<Weather> {
        println!("OpenWeather for_coordinates start {request:?}");
        let url = Url::parse_with_params(
            ENDPOINT_URL,
            &[
                ("lat", request.query.get_latitude().to_string()),
                ("lon", request.query.get_longitude().to_string()),
                ("appid", self.api_key.to_owned()),
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

        println!("OpenWeather for_coordinates end {request:?}");
        Ok(Weather {
            source: SOURCE_URI.to_string(),
            location: request.name.clone(),
            city: response.name,
            temperature: response.main.temp.to_celsius(),
            coordinates: response.coord,
        })
    }

    fn cache_lifetime(&self) -> Duration {
        self.cache.refresh_interval
    }
}
