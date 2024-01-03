use crate::providers::http_request::{request_cached, HttpCacheRequest};
use crate::providers::units::Coordinates;
use crate::providers::HttpRequestCache;
use crate::providers::{Weather, WeatherProvider, WeatherRequest};
use anyhow::format_err;
use reqwest::blocking::Client;
use reqwest::{Method, Url};
use rocket::serde::Serialize;
use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Nogoodnik;

const SOURCE_URI: &str = "local.nogoodnik";

impl WeatherProvider for Nogoodnik {
    fn id(&self) -> &str {
        SOURCE_URI
    }

    fn for_coordinates(
        &self,
        client: &Client,
        cache: &HttpRequestCache,
        _request: &WeatherRequest<Coordinates>,
    ) -> anyhow::Result<Weather> {
        request_cached(&HttpCacheRequest::new_json_request(
            SOURCE_URI,
            client,
            cache,
            &Method::GET,
            &Url::parse("http://example.org/404")?,
        ))?;

        Err(format_err!("This provider is no good and always fails"))
    }

    fn refresh_interval(&self) -> Duration {
        Duration::from_secs(0)
    }
}
