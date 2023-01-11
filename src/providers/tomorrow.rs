use crate::providers::cache::{reqwest_cached_body_json, Configuration, RequestBody};
use crate::providers::units::{Celsius, Coordinates, Ratio};
use crate::providers::{Weather, WeatherProvider, WeatherRequest};
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
        cache: &RequestBody,
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

        let client = reqwest::blocking::Client::new();

        let response = reqwest_cached_body_json::<TomorrowResponse>(
            SOURCE_URI,
            cache,
            &client,
            Method::GET,
            url,
            None,
        )?;

        let values = &response
            .data
            .timelines
            .get(0)
            .expect("Timelines cannot be empty")
            .intervals
            .get(0)
            .expect("Intervals cannot be empty")
            .values;

        Ok(Weather {
            source: SOURCE_URI.into(),
            location: request.name.clone(),
            city: request.name.clone(),
            temperature: values.temperature,
            relative_humidity: Some(values.humidity),
            coordinates: request.query.clone(),
        })
    }

    fn refresh_interval(&self) -> Duration {
        self.cache.refresh_interval
    }
}
