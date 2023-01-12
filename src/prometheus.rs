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

    let mut registry = <prometheus_client::registry::Registry>::with_prefix("weather");

    let temperature = Family::<Labels, Gauge<f64, AtomicU64>>::default();
    registry.register_with_unit(
        "temperature",
        format!("{NAME} temperature"),
        Unit::Celsius,
        temperature.clone(),
    );

    let humidity = Family::<Labels, Gauge<f64, AtomicU64>>::default();
    let mut humidity_registered = false;

    for weather in weathers {
        let labels = Labels {
            version: VERSION.into(),
            source: weather.source,
            location: weather.location,
            city: weather.city,
            latitude: weather.coordinates.latitude.to_string(),
            longitude: weather.coordinates.longitude.to_string(),
        };

        temperature
            .get_or_create(&labels)
            .set(weather.temperature.into());

        weather.relative_humidity.map(|rh| {
            if !humidity_registered {
                registry.register_with_unit(
                    "relative_humidity",
                    format!("{NAME} relative humidity"),
                    Unit::Other("ratio".into()),
                    humidity.clone(),
                );
                humidity_registered = true;
            }

            humidity.get_or_create(&labels).set(rh.as_f64())
        });
    }

    let mut buffer = String::new();

    encode(&mut buffer, &registry)?;

    Ok(buffer)
}

#[cfg(test)]
mod tests {
    use crate::prometheus::format;
    use crate::providers::units::Ratio::Ratio;
    use crate::providers::units::{Celsius, Coordinate, Coordinates};
    use crate::providers::Weather;
    use assert_str::assert_str_eq;

    #[test]
    fn format_single_temperature() {
        assert_str_eq!(
            format!(
                r##"# HELP weather_temperature_celsius prometheus-weathermen temperature.
# TYPE weather_temperature_celsius gauge
# UNIT weather_temperature_celsius celsius
weather_temperature_celsius{{version="{0}",source="org.example",location="My Name",city="Some City",latitude="20.1000000",longitude="10.0123400"}} 25.5
# EOF
"##,
                crate::config::VERSION
            ),
            format(vec![Weather {
                source: "org.example".into(),
                coordinates: Coordinates {
                    latitude: Coordinate::from(20.1),
                    longitude: Coordinate::from(10.01234),
                },
                location: "My Name".into(),
                city: "Some City".into(),
                temperature: Celsius::from(25.5),
                relative_humidity: None
            }])
            .expect("Formatting should work")
        )
    }

    #[test]
    fn format_temperature_and_humidity() {
        assert_str_eq!(
            format!(
                r##"# HELP weather_temperature_celsius prometheus-weathermen temperature.
# TYPE weather_temperature_celsius gauge
# UNIT weather_temperature_celsius celsius
weather_temperature_celsius{{version="{0}",source="org.example",location="My Name",city="Some City",latitude="20.1000000",longitude="10.0123400"}} 25.5
# HELP weather_relative_humidity_ratio prometheus-weathermen relative humidity.
# TYPE weather_relative_humidity_ratio gauge
# UNIT weather_relative_humidity_ratio ratio
weather_relative_humidity_ratio{{version="{0}",source="org.example",location="My Name",city="Some City",latitude="20.1000000",longitude="10.0123400"}} 0.55
# EOF
"##,
                crate::config::VERSION
            ),
            format(vec![Weather {
                source: "org.example".into(),
                coordinates: Coordinates {
                    latitude: Coordinate::from(20.1),
                    longitude: Coordinate::from(10.01234),
                },
                location: "My Name".into(),
                city: "Some City".into(),
                temperature: Celsius::from(25.5),
                relative_humidity: Some(Ratio(0.55))
            }])
            .expect("Formatting should work")
        )
    }

    #[test]
    #[ignore] // Sort order of metrics is not deterministic
    fn format_multiple() {
        assert_str_eq!(
            format!(
                r##"# HELP weather_temperature_celsius prometheus-weathermen temperature.
# TYPE weather_temperature_celsius gauge
# UNIT weather_temperature_celsius celsius
weather_temperature_celsius{{version="{0}",source="org.example",location="My Name",city="Some City",latitude="20.1000004",longitude="10.0123396"}} 25.5
weather_temperature_celsius{{version="{0}",source="com.example",location="Another Name",city="Another City",latitude="30.1000004",longitude="20.0123405"}} 15.5
# HELP weather_relative_humidity_ratio prometheus-weathermen relative humidity.
# TYPE weather_relative_humidity_ratio gauge
# UNIT weather_relative_humidity_ratio ratio
weather_relative_humidity_ratio{{version="{0}",source="org.example",location="My Name",city="Some City",latitude="20.1000004",longitude="10.0123396"}} 0.55
weather_relative_humidity_ratio{{version="{0}",source="com.example",location="Another Name",city="Another City",latitude="30.1000004",longitude="20.0123405"}} 0.75
# EOF
"##,
                crate::config::VERSION
            ),
            format(vec![
                Weather {
                    source: "org.example".into(),
                    coordinates: Coordinates {
                        latitude: Coordinate::from(20.1),
                        longitude: Coordinate::from(10.01234),
                    },
                    location: "My Name".into(),
                    city: "Some City".into(),
                    temperature: Celsius::from(25.5),
                    relative_humidity: Some(Ratio(0.55))
                },
                Weather {
                    source: "com.example".into(),
                    coordinates: Coordinates {
                        latitude: Coordinate::from(30.1),
                        longitude: Coordinate::from(20.01234),
                    },
                    location: "Another Name".into(),
                    city: "Another City".into(),
                    temperature: Celsius::from(15.5),
                    relative_humidity: Some(Ratio(0.75))
                }
            ])
            .expect("Formatting should work")
        )
    }
}
