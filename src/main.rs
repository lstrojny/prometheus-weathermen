use crate::prometheus::prometheus_metrics;
use rocket::tokio::task;
use rocket::{get, launch, routes};
use std::env;

mod prometheus;
mod provider;
use crate::provider::open_weather::OpenWeather;
use crate::provider::provider::{Coordinate, Coordinates, WeatherProvider};

#[get("/")]
async fn index() -> String {
    let coordinates = Coordinates::new(
        Coordinate::new(48.137154_f32),
        Coordinate::new(11.576124_f32),
    );

    let api_key = match env::var("OPENWEATHER_API_KEY").ok() {
        Some(string) => string,
        None => return "# No API key given".to_owned(),
    };

    let provider = OpenWeather { api_key };

    let provide_handle =
        task::spawn_blocking(move || provider.for_coordinates(coordinates.to_owned()));

    let weather = match provide_handle.await {
        Ok(weather) => match weather {
            Ok(weather) => weather,
            Err(err) => return format!("# {}", err),
        },
        Err(err) => return err.to_string().to_owned(),
    };

    return prometheus_metrics(weather);
}

#[launch]
fn rocket() -> _ {
    rocket::build().mount("/", routes![index])
}
