use crate::providers::http_request::{request_cached, Configuration, HttpCacheRequest};
use crate::providers::units::{Celsius, Coordinates};
use crate::providers::{
    calculate_distance, HttpRequestCache, Weather, WeatherProvider, WeatherRequest,
};
use hmac::{Hmac, Mac};
use reqwest::blocking::Client;
use reqwest::{Method, Url};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::time::Duration;

type HmacSha256 = Hmac<Sha256>;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Meteoblue {
    api_key: String,
    #[serde(flatten)]
    cache: Configuration,
}

const SOURCE_URI: &str = "com.meteoblue";
const ENDPOINT_URL: &str = "https://my.meteoblue.com/packages/current";

#[derive(Deserialize, Debug)]
struct MeteoblueResponseMetadata {
    name: String,
    #[serde(flatten)]
    coordinates: Coordinates,
}

#[derive(Deserialize, Debug)]
struct MeteoblueResponseDataCurrent {
    temperature: Celsius,
}

#[derive(Deserialize, Debug)]
struct MeteoblueResponse {
    metadata: MeteoblueResponseMetadata,
    data_current: MeteoblueResponseDataCurrent,
}

impl WeatherProvider for Meteoblue {
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
                ("format", "json".into()),
                ("apikey", self.api_key.clone()),
            ],
        )?;

        let sig = create_signature(&self.api_key, &url)?;

        let signed_url = Url::parse_with_params(url.as_str(), &[("sig", sig)])?;

        let response: MeteoblueResponse = request_cached(&HttpCacheRequest::new_json_request(
            SOURCE_URI,
            client,
            cache,
            &Method::GET,
            &signed_url,
        ))?;

        let distance = calculate_distance(&request.query, &response.metadata.coordinates);

        Ok(Weather {
            source: SOURCE_URI.into(),
            location: request.name.clone(),
            city: if response.metadata.name.is_empty() {
                request.name.clone()
            } else {
                response.metadata.name
            },
            coordinates: response.metadata.coordinates,
            distance: Some(distance),
            temperature: response.data_current.temperature,
            relative_humidity: None,
        })
    }

    fn refresh_interval(&self) -> Duration {
        self.cache.refresh_interval
    }
}

fn create_signature(api_key: &str, url: &Url) -> anyhow::Result<String> {
    let mut mac = HmacSha256::new_from_slice(api_key.as_bytes())?;

    mac.update(url.path().as_bytes());
    mac.update(b"?");
    mac.update(
        url.query()
            .expect("Query cannot be empty as it was set above")
            .as_bytes(),
    );
    let key = mac.finalize();

    Ok(hex::encode(key.into_bytes()))
}
