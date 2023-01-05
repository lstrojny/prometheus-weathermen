use crate::providers::units::{Kelvin, ToCelsius};
use crate::providers::{Coordinates, Weather, WeatherProvider, WeatherRequest};
use reqwest::{Method, Url};
use rocket::serde::Deserialize;
use std::string::ToString;

const SOURCE_URI: &str = "org.openweathermap";
const ENDPOINT_URL: &str = "https://api.openweathermap.org/data/2.5/weather";

#[derive(Deserialize, Debug, Clone)]
pub struct OpenWeather {
    pub api_key: String,
}

#[derive(Deserialize)]
struct OpenWeatherResponseMain {
    temp: Kelvin,
}

#[derive(Deserialize)]
struct OpenWeatherResponse {
    coord: Coordinates,
    name: String,
    main: OpenWeatherResponseMain,
}

impl WeatherProvider for OpenWeather {
    fn for_coordinates(&self, request: WeatherRequest<Coordinates>) -> Result<Weather, String> {
        println!("OpenWeather for_coordinates start {request:?}");
        let url = match Url::parse_with_params(
            ENDPOINT_URL,
            &[
                ("lat", request.query.get_latitude().to_string()),
                ("lon", request.query.get_longitude().to_string()),
                ("appid", self.api_key.to_owned()),
            ],
        ) {
            Ok(url) => url,
            Err(e) => return Err(e.to_string()),
        };

        let client = reqwest::blocking::Client::new();
        let request_builder = client.request(Method::GET, url).send();

        let response = match request_builder {
            Ok(response) => match response.json::<OpenWeatherResponse>() {
                Ok(response) => response,
                Err(err) => return Err(err.to_string()),
            },
            Err(err) => return Err(err.to_string()),
        };

        println!("OpenWeather for_coordinates end {request:?}");
        Ok(Weather {
            source: SOURCE_URI.to_string(),
            location: request.name,
            city: response.name,
            temperature: response.main.temp.to_celsius(),
            coordinates: response.coord,
        })
    }
}
