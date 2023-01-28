use crate::providers::http_request::{request_cached, Configuration, HttpCacheRequest};
use crate::providers::units::{Coordinates, Kelvin, Ratio, ToCelsius};
use crate::providers::{HttpRequestCache, Weather, WeatherProvider, WeatherRequest};
use reqwest::blocking::Client;
use reqwest::{Method, Url};
use rocket::serde::Deserialize;
use serde::Serialize;
use std::fmt::Debug;
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
        client: &Client,
        cache: &HttpRequestCache,
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

        let response: OpenWeatherResponse = request_cached(&HttpCacheRequest::new_json_request(
            SOURCE_URI,
            client,
            cache,
            &Method::GET,
            &url,
        ))?;

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
