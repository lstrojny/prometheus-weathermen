use opentelemetry::sdk::export::metrics::aggregation;
use opentelemetry::sdk::metrics::{controllers, processors, selectors};
use opentelemetry::sdk::Resource;
use opentelemetry::{global, Context, KeyValue};
use prometheus::{Encoder, TextEncoder};
use rocket::tokio::task;
use rocket::{get, launch, routes};
use std::env;

mod provider;
use crate::provider::open_weather::OpenWeather;
use crate::provider::provider::{Coordinate, Coordinates, WeatherProvider};

#[get("/")]
async fn index() -> String {
    let controller = controllers::basic(processors::factory(
        selectors::simple::inexpensive(),
        aggregation::stateless_temporality_selector(),
    ))
    .with_resource(Resource::new(vec![KeyValue::new(
        "service.name",
        "prometheus-weather-exporter",
    )]))
    .build();
    let exporter = opentelemetry_prometheus::exporter(controller).init();
    let cx = Context::current();
    let meter = global::meter("foo.com/prometheus-weather-exporter");

    let temperature = meter
        .f64_up_down_counter("weather_temperature_celsius")
        .with_description("Temperature in celsius")
        .init();

    let coordinates = Coordinates::new(
        Coordinate::new(48.137154_f32),
        Coordinate::new(11.576124_f32),
    );

    let provider = OpenWeather {
        api_key: env::var("OPENWEATHER_API_KEY").ok().unwrap(),
    };

    let new_coordinates = coordinates.clone();
    let weather = task::spawn_blocking(move || provider.for_coordinates(new_coordinates))
        .await
        .unwrap();

    temperature.add(
        &cx,
        *weather.temperature as f64,
        &[
            KeyValue::new("latitude", coordinates.clone().get_latitude().to_string()),
            KeyValue::new("longitude", coordinates.clone().get_longitude().to_string()),
        ],
    );

    let encoder = TextEncoder::new();
    let metric_families = exporter.registry().gather();
    let mut result = Vec::new();
    encoder.encode(&metric_families, &mut result).unwrap();

    return String::from_utf8(result).unwrap();
}

#[launch]
fn rocket() -> _ {
    rocket::build().mount("/", routes![index])
}
