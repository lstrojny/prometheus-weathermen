use crate::provider::provider::{Coordinates, Weather, WeatherProvider};
use crate::provider::units::{Kelvin, ToCelsius};
use reqwest::{Method, Url};
use rocket::serde::Deserialize;

pub struct OpenWeather {
    pub api_key: String,
}

#[derive(Deserialize)]
struct Main {
    temp: Kelvin,
}

#[derive(Deserialize)]
struct OpenWeatherResponse {
    main: Main,
}

impl WeatherProvider for OpenWeather {
    fn for_coordinates(&self, coordinates: Coordinates) -> Weather {
        let url = Url::parse_with_params(
            "https://api.openweathermap.org/data/2.5/weather",
            &[
                ("lat", coordinates.get_latitude().to_string()),
                ("lon", coordinates.get_longitude().to_string()),
                ("appid", self.api_key.clone()),
            ],
        );

        let client = reqwest::blocking::Client::new();
        let r = client.request(Method::GET, url.unwrap()).send();

        let response = r.unwrap().json::<OpenWeatherResponse>();

        return Weather {
            temperature: response.unwrap().main.temp.to_celsius(),
            coordinates: Coordinates::new(
                coordinates.get_latitude().clone(),
                coordinates.get_longitude().clone(),
            ),
        };

        // ?lat=48.137154&lon=11.576124&appid=

        /*println!("{}", body);*/
    }
}
