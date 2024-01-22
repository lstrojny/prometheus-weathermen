use crate::providers::http_request::{request_cached, Configuration, HttpCacheRequest};
use crate::providers::units::Coordinates;
use crate::providers::units::Ratio::Percentage;
use crate::providers::{HttpRequestCache, Weather, WeatherProvider, WeatherRequest};
use reqwest::blocking::Client;
use reqwest::{Method, Url};
use rocket::serde::{Deserialize, Serialize};
use std::time::Duration;

const SOURCE_URI: &str = "com.open-meteo";

const ENDPOINT_URL: &str = "https://api.open-meteo.com/v1/forecast";

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OpenMeteo {
    api_key: Option<String>,
    #[serde(flatten)]
    cache: Configuration,
}

#[derive(Deserialize, Debug)]
struct OpenMeteoResponse {
    current: OpenMeteoResponseCurrent,
}

#[derive(Deserialize, Debug)]
struct OpenMeteoResponseCurrent {
    temperature_2m: f32,
    relative_humidity_2m: f64,
}

impl WeatherProvider for OpenMeteo {
    fn id(&self) -> &str {
        SOURCE_URI
    }

    fn for_coordinates(
        &self,
        client: &Client,
        cache: &HttpRequestCache,
        request: &WeatherRequest<Coordinates>,
    ) -> anyhow::Result<Weather> {
        let mut url = Url::parse_with_params(
            ENDPOINT_URL,
            &[
                ("current", "temperature_2m,relative_humidity_2m".to_owned()),
                ("latitude", request.query.latitude.to_string()),
                ("longitude", request.query.longitude.to_string()),
            ],
        )?;

        if let Some(api_key) = &self.api_key {
            url.query_pairs_mut().append_pair("apikey", api_key);
        }

        let response: OpenMeteoResponse = request_cached(&HttpCacheRequest::new_json_request(
            SOURCE_URI,
            client,
            cache,
            &Method::GET,
            &url,
        ))?;

        Ok(Weather {
            coordinates: request.query.clone(),
            source: SOURCE_URI.into(),
            location: request.name.clone(),
            city: None,
            distance: None,
            temperature: response.current.temperature_2m.into(),
            relative_humidity: Some(Percentage(response.current.relative_humidity_2m)),
        })
    }

    fn refresh_interval(&self) -> Duration {
        Duration::from_secs(900)
    }
}
