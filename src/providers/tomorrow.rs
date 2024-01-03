use crate::providers::http_request::{request_cached, Configuration, HttpCacheRequest};
use crate::providers::units::{Celsius, Coordinates, Ratio};
use crate::providers::{HttpRequestCache, Weather, WeatherProvider, WeatherRequest};
use reqwest::blocking::Client;
use reqwest::{Method, Url};
use serde::{Deserialize, Serialize};
use std::time::Duration;

const SOURCE_URI: &str = "io.tomorrow";
const ENDPOINT_URL: &str = "https://api.tomorrow.io/v4/weather/realtime";

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Tomorrow {
    api_key: String,
    #[serde(flatten)]
    cache: Configuration,
}

#[derive(Deserialize, Debug)]
struct TomorrowResponse {
    data: TomorrowData,
}

#[derive(Deserialize, Debug)]
struct TomorrowData {
    values: TomorrowValues,
}

#[derive(Deserialize, Debug)]
struct TomorrowValues {
    temperature: Celsius,
    humidity: Ratio,
}

impl WeatherProvider for Tomorrow {
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
                (
                    "location",
                    format!("{},{}", request.query.latitude, request.query.longitude),
                ),
                ("apikey", self.api_key.clone()),
                ("units", "metric".into()),
            ],
        )?;

        let response: TomorrowResponse = request_cached(&HttpCacheRequest::new_json_request(
            SOURCE_URI,
            client,
            cache,
            &Method::GET,
            &url,
        ))?;

        Ok(Weather {
            source: SOURCE_URI.into(),
            location: request.name.clone(),
            city: request.name.clone(),
            coordinates: request.query.clone(),
            distance: None,
            temperature: response.data.values.temperature,
            relative_humidity: Some(response.data.values.humidity),
        })
    }

    fn refresh_interval(&self) -> Duration {
        self.cache.refresh_interval
    }
}
