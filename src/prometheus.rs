use crate::config::{NAME, VERSION};
use crate::providers::Weather;
use log::debug;
use prometheus_client::encoding::text::encode;
use prometheus_client::encoding::EncodeLabelSet;
use prometheus_client::metrics::family::Family;
use prometheus_client::metrics::gauge::Gauge;
use prometheus_client::registry::Unit;
use std::sync::atomic::AtomicU64;

#[derive(Clone, Hash, Eq, PartialEq, EncodeLabelSet, Debug)]
struct Labels {
    version: String,
    source: String,
    location: String,
    city: String,
    latitude: String,
    longitude: String,
}

pub fn format(weathers: Vec<Weather>) -> anyhow::Result<String> {
    debug!("Formatting {weathers:?}");

    let mut registry = <prometheus_client::registry::Registry>::default();

    let temperature = Family::<Labels, Gauge<f64, AtomicU64>>::default();
    registry.register_with_unit(
        "weather_temperature",
        format!("{NAME} temperature"),
        Unit::Celsius,
        temperature.clone(),
    );

    let humidity = Family::<Labels, Gauge<f64, AtomicU64>>::default();
    registry.register_with_unit(
        "weather_relative_humidity",
        format!("{NAME} relative humidity"),
        Unit::Ratios,
        humidity.clone(),
    );

    for weather in weathers {
        let labels = &Labels {
            version: VERSION.into(),
            source: weather.source,
            location: weather.location,
            city: weather.city,
            latitude: weather.coordinates.latitude.to_string(),
            longitude: weather.coordinates.longitude.to_string(),
        };

        temperature
            .get_or_create(labels)
            .set(weather.temperature.into());

        weather
            .relative_humidity
            .map(|rh| humidity.get_or_create(labels).set(rh.as_f64()));
    }

    let mut buffer = String::new();

    encode(&mut buffer, &registry)?;

    Ok(buffer)
}
