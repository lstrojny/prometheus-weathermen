use crate::providers::units::Celsius;
use crate::providers::{Coordinates, Weather, WeatherProvider, WeatherRequest};
use hmac::{Hmac, Mac};
use reqwest::{Method, Url};
use serde::Deserialize;
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

#[derive(Deserialize, Debug, Clone)]
pub struct Meteoblue {
    pub api_key: String,
}

const SOURCE_URI: &str = "com.meteoblue";
const ENDPOINT_URL: &str = "https://my.meteoblue.com/packages/current";

#[derive(Deserialize)]
struct MeteoblueResponseMetadata {
    name: String,
    #[serde(flatten)]
    coordinates: Coordinates,
}

#[derive(Deserialize)]
struct MeteoblueResponseDataCurrent {
    temperature: Celsius,
}

#[derive(Deserialize)]
struct MeteoblueResponse {
    metadata: MeteoblueResponseMetadata,
    data_current: MeteoblueResponseDataCurrent,
}

impl WeatherProvider for Meteoblue {
    fn for_coordinates(&self, request: WeatherRequest) -> Result<Weather, String> {
        println!("Meteoblue for_coordinates start {request:?}");

        let url = match Url::parse_with_params(
            ENDPOINT_URL,
            &[
                ("forecast_days", "1".to_string()),
                ("history_days", "0".to_string()),
                ("lat", request.coordinates.get_latitude().to_string()),
                ("lon", request.coordinates.get_longitude().to_string()),
                ("format", "json".to_string()),
                ("apikey", self.api_key.clone()),
            ],
        ) {
            Ok(url) => url,
            Err(e) => return Err(e.to_string()),
        };

        let mut mac = HmacSha256::new_from_slice(self.api_key.as_bytes())
            .expect("HMAC can take key of any size");

        mac.update(url.path().as_bytes());
        mac.update("?".as_bytes());
        mac.update(url.query().unwrap().as_bytes());
        let key = mac.finalize();

        let sig = hex::encode(key.into_bytes());

        let signed_url = Url::parse_with_params(url.as_str(), &[("sig", sig)]).unwrap();
        println!("Signed URL {:?}", signed_url.to_string());

        let client = reqwest::blocking::Client::new();
        let request_builder = client.request(Method::GET, signed_url).send();

        let response = match request_builder {
            Ok(response) => match response.json::<MeteoblueResponse>() {
                Ok(response) => response,
                Err(err) => return Err(err.to_string()),
            },
            Err(err) => return Err(err.to_string()),
        };

        println!("Meteoblue for_coordinates end {request:?}");
        Ok(Weather {
            source: SOURCE_URI.to_string(),
            location: request.name.clone(),
            city: match response.metadata.name.is_empty() {
                true => request.name,
                false => response.metadata.name,
            },
            temperature: response.data_current.temperature,
            coordinates: response.metadata.coordinates,
        })
    }
}
