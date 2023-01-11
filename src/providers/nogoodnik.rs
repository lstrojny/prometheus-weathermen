use crate::providers::cache::RequestBody;
use crate::providers::units::Coordinates;
use crate::providers::{Weather, WeatherProvider, WeatherRequest};
use anyhow::format_err;
use rocket::serde::Serialize;
use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Nogoodnik {}

const SOURCE_URI: &str = "local.nogoodnik";

impl WeatherProvider for Nogoodnik {
    fn id(&self) -> &str {
        SOURCE_URI
    }

    fn for_coordinates(
        &self,
        _cache: &RequestBody,
        _request: &WeatherRequest<Coordinates>,
    ) -> anyhow::Result<Weather> {
        Err(format_err!("This provider is no good and always fails"))
    }

    fn refresh_interval(&self) -> Duration {
        Duration::from_secs(0)
    }
}
