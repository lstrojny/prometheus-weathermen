use crate::provider::provider::{Coordinate, Coordinates, Weather, WeatherProvider};
use crate::provider::units::{Kelvin, ToCelsius};
use reqwest::{Method, Url};
use rocket::serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct OpenWeather {
    pub api_key: String,
}

#[derive(Deserialize)]
struct OpenWeatherResponseMain {
    temp: Kelvin,
}

#[derive(Deserialize)]
struct OpenWeatherResponseCoord {
    lat: Coordinate,
    lon: Coordinate,
}

#[derive(Deserialize)]
struct OpenWeatherResponse {
    coord: OpenWeatherResponseCoord,
    name: String,
    main: OpenWeatherResponseMain,
}

impl WeatherProvider for OpenWeather {
    fn for_coordinates(&self, coordinates: Coordinates) -> Result<Weather, String> {
        println!("OpenWeather for_coordinates start {:?}", coordinates);
        let url = match Url::parse_with_params(
            "https://api.openweathermap.org/data/2.5/weather",
            &[
                ("lat", coordinates.get_latitude().to_string()),
                ("lon", coordinates.get_longitude().to_string()),
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

        println!("OpenWeather for_coordinates end {:?}", coordinates);
        return Ok(Weather {
            city: response.name,
            temperature: response.main.temp.to_celsius(),
            coordinates: Coordinates::new(response.coord.lat, response.coord.lon),
        });
    }
}
