use crate::providers::cache::{reqwest_cached_body, Configuration, RequestBody};
use crate::providers::units::Coordinates;
use crate::providers::{Weather, WeatherProvider, WeatherRequest};
use const_format::concatcp;
use csv::Trim;
use log::debug;
use reqwest::blocking::Client;
use reqwest::{Method, Url};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Dwd {
    #[serde(flatten)]
    pub cache: Configuration,
}

const SOURCE_URI: &str = "de.dwd";

const BASE_URL: &str = "https://opendata.dwd.de/climate_environment/CDC/observations_germany/climate/10_minutes/air_temperature/recent/";
const METADATA_URL: &str = concatcp!(BASE_URL, "zehn_min_tu_Beschreibung_Stationen.txt");

impl WeatherProvider for Dwd {
    fn id(&self) -> &str {
        SOURCE_URI
    }

    fn for_coordinates(
        &self,
        cache: &RequestBody,
        request: &WeatherRequest<Coordinates>,
    ) -> anyhow::Result<Weather> {
        let client = Client::new();

        let re = regex::Regex::new(r"[ ]+")?;

        let metadata = reqwest_cached_body(
            SOURCE_URI,
            cache,
            &client,
            Method::GET,
            Url::parse(METADATA_URL)?,
            Some("iso-8859-15"),
        )?;

        let metadata_clean = re.replace_all(&metadata, " ");

        debug!("{}", metadata_clean);

        let mut reader = csv::ReaderBuilder::new()
            .delimiter(b' ')
            .double_quote(false)
            .comment(Some(b'-'))
            .trim(Trim::All)
            .flexible(true)
            .from_reader(metadata_clean.as_bytes());
        for result in reader.records() {
            // The iterator yields Result<StringRecord, Error>, so we check the
            // error here.
            let record = result?;
            println!("{:?}", record);
        }

        todo!()
    }

    fn refresh_interval(&self) -> Duration {
        self.cache.refresh_interval
    }
}
