use crate::providers::cache::{request_cached, Configuration, HttpCacheRequest};
use crate::providers::units::{Celsius, Coordinates, Ratio};
use crate::providers::{HttpRequestBodyCache, Weather, WeatherProvider, WeatherRequest};
use anyhow::anyhow;
use reqwest::blocking::Client;
use reqwest::{Method, Url};
use serde::{Deserialize, Serialize};
use std::time::Duration;

const SOURCE_URI: &str = "io.tomorrow";
const ENDPOINT_URL: &str = "https://api.tomorrow.io/v4/timelines";

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Tomorrow {
    pub api_key: String,
    #[serde(flatten)]
    pub cache: Configuration,
}

#[derive(Deserialize, Debug)]
struct TomorrowResponse {
    data: TomorrowData,
}

#[derive(Deserialize, Debug)]
struct TomorrowData {
    timelines: Vec<TomorrowTimeline>,
}

#[derive(Deserialize, Debug)]
struct TomorrowTimeline {
    intervals: Vec<TomorrowInterval>,
}

#[derive(Deserialize, Debug)]
struct TomorrowInterval {
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
        cache: &HttpRequestBodyCache,
        request: &WeatherRequest<Coordinates>,
    ) -> anyhow::Result<Weather> {
        let url = Url::parse_with_params(
            ENDPOINT_URL,
            &[
                (
                    "location",
                    format!("{},{}", request.query.latitude, request.query.longitude),
                ),
                ("apikey", self.api_key.to_string()),
                ("fields", "temperature,humidity".into()),
                ("units", "metric".into()),
                ("timesteps", "1m".into()),
                ("startTime", "now".into()),
                ("endTime", "nowPlus1m".into()),
            ],
        )?;

        let response: TomorrowResponse = request_cached(&HttpCacheRequest::new_json_request(
            SOURCE_URI,
            client,
            cache,
            &Method::GET,
            &url,
        ))?;

        match &response.data.timelines[..] {
            [timeline, ..] => match &timeline.intervals[..] {
                [interval, ..] => Ok(Weather {
                    source: SOURCE_URI.into(),
                    location: request.name.clone(),
                    city: request.name.clone(),
                    temperature: interval.values.temperature,
                    relative_humidity: Some(interval.values.humidity),
                    coordinates: request.query.clone(),
                }),
                [] => Err(anyhow!("Empty intervals in response")),
            },
            [] => Err(anyhow!("Empty timelines in response")),
        }
    }

    fn refresh_interval(&self) -> Duration {
        self.cache.refresh_interval
    }
}
