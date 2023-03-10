use crate::config::{NAME, VERSION};
use crate::providers::Weather;
use log::debug;
use prometheus_client::encoding::text::encode;
use prometheus_client::encoding::EncodeLabelSet;
use prometheus_client::metrics::family::Family;
use prometheus_client::metrics::gauge::Gauge;
use prometheus_client::registry::{Registry, Unit};
use std::sync::atomic::AtomicU64;

#[derive(PartialEq, Debug, Eq, Copy, Clone)]
pub enum Format {
    Prometheus,
    OpenMetrics,
}

#[derive(Clone, Hash, Eq, PartialEq, EncodeLabelSet, Debug)]
struct Labels {
    version: String,
    source: String,
    location: String,
    city: String,
    latitude: String,
    longitude: String,
}

pub fn format_metrics(_format: Format, weathers: Vec<Weather>) -> anyhow::Result<String> {
    debug!("Formatting {weathers:?}");

    let mut registry = Registry::with_prefix("weather");

    let temperature = Family::<Labels, Gauge<f64, AtomicU64>>::default();
    registry.register_with_unit(
        "temperature",
        format!("{NAME} temperature"),
        Unit::Celsius,
        temperature.clone(),
    );

    let humidity = Family::<Labels, Gauge<f64, AtomicU64>>::default();
    let mut humidity_registered = false;

    let station_distance = Family::<Labels, Gauge<f64, AtomicU64>>::default();
    let mut station_distance_registered = false;

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

        weather.relative_humidity.map(|relative_humidity_ratio| {
            if !humidity_registered {
                registry.register_with_unit(
                    "relative_humidity",
                    format!("{NAME} relative humidity"),
                    Unit::Other("ratio".into()),
                    humidity.clone(),
                );
                humidity_registered = true;
            }

            humidity
                .get_or_create(&labels)
                .set(relative_humidity_ratio.as_f64())
        });

        weather.distance.map(|meters| {
            if !station_distance_registered {
                registry.register_with_unit(
                    "station_distance",
                    format!("{NAME} weather station distance in meters"),
                    Unit::Meters,
                    station_distance.clone(),
                );
                station_distance_registered = true;
            }

            station_distance.get_or_create(&labels).set(meters.into())
        });
    }

    let mut buffer = String::new();

    encode(&mut buffer, &registry)?;

    Ok(buffer)
}

#[cfg(test)]
mod tests {
    use crate::prometheus::{format_metrics, Format};
    use crate::providers::units::Ratio::Ratio;
    use crate::providers::units::{Celsius, Coordinate, Coordinates};
    use crate::providers::Weather;
    use pretty_assertions::assert_str_eq;
    use std::cmp::Ordering;

    fn sort_output_deterministically(output: &str) -> String {
        let mut lines: Vec<&str> = output.lines().collect();

        lines.sort_by(|left, right| {
            let left_is_comment = left.starts_with('#');
            let right_is_comment = right.starts_with('#');
            let left_metric_id = get_metric_identifier(left, left_is_comment);
            let right_metric_id = get_metric_identifier(right, right_is_comment);

            // We only sort the metrics themselves and leave the rest untouched
            if left_is_comment && right_is_comment || left_metric_id != right_metric_id {
                return Ordering::Equal;
            }

            left.partial_cmp(right).unwrap_or(Ordering::Equal)
        });

        lines.join("\n")
    }

    fn get_metric_identifier(line: &str, is_comment: bool) -> String {
        if is_comment {
            line.split(' ').nth(2).unwrap_or("").into()
        } else {
            line.split('{')
                .next()
                .expect("Could not extract identifier from metric line")
                .into()
        }
    }

    #[test]
    fn format_single_temperature() {
        assert_str_eq!(
            format!(
                r##"# HELP weather_temperature_celsius prometheus-weathermen temperature.
# TYPE weather_temperature_celsius gauge
# UNIT weather_temperature_celsius celsius
weather_temperature_celsius{{version="{0}",source="org.example",location="My Name",city="Some City",latitude="20.1000000",longitude="10.0123400"}} 25.5
# EOF"##,
                crate::config::VERSION
            ),
            sort_output_deterministically(
                &format_metrics(
                    Format::Prometheus,
                    vec![Weather {
                        source: "org.example".into(),
                        coordinates: Coordinates {
                            latitude: Coordinate::from(20.1),
                            longitude: Coordinate::from(10.01234),
                        },
                        location: "My Name".into(),
                        city: "Some City".into(),
                        temperature: Celsius::from(25.5),
                        relative_humidity: None,
                        distance: None,
                    }]
                )
                .expect("Formatting should work")
            )
        );
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
# EOF"##,
                crate::config::VERSION
            ),
            sort_output_deterministically(
                &format_metrics(
                    Format::Prometheus,
                    vec![Weather {
                        source: "org.example".into(),
                        coordinates: Coordinates {
                            latitude: Coordinate::from(20.1),
                            longitude: Coordinate::from(10.01234),
                        },
                        location: "My Name".into(),
                        city: "Some City".into(),
                        temperature: Celsius::from(25.5),
                        relative_humidity: Some(Ratio(0.55)),
                        distance: None,
                    }]
                )
                .expect("Formatting should work")
            )
        );
    }

    #[test]
    fn format_multiple() {
        assert_str_eq!(
            format!(
                r##"# HELP weather_temperature_celsius prometheus-weathermen temperature.
# TYPE weather_temperature_celsius gauge
# UNIT weather_temperature_celsius celsius
weather_temperature_celsius{{version="{0}",source="com.example",location="Another Name",city="Another City",latitude="30.1000000",longitude="20.0123400"}} 15.5
weather_temperature_celsius{{version="{0}",source="org.example",location="My Name",city="Some City",latitude="20.1000000",longitude="10.0123400"}} 25.5
# HELP weather_relative_humidity_ratio prometheus-weathermen relative humidity.
# TYPE weather_relative_humidity_ratio gauge
# UNIT weather_relative_humidity_ratio ratio
weather_relative_humidity_ratio{{version="{0}",source="com.example",location="Another Name",city="Another City",latitude="30.1000000",longitude="20.0123400"}} 0.75
weather_relative_humidity_ratio{{version="{0}",source="org.example",location="My Name",city="Some City",latitude="20.1000000",longitude="10.0123400"}} 0.55
# EOF"##,
                crate::config::VERSION
            ),
            sort_output_deterministically(
                &format_metrics(
                    Format::Prometheus,
                    vec![
                        Weather {
                            source: "org.example".into(),
                            coordinates: Coordinates {
                                latitude: Coordinate::from(20.1),
                                longitude: Coordinate::from(10.01234),
                            },
                            location: "My Name".into(),
                            city: "Some City".into(),
                            temperature: Celsius::from(25.5),
                            relative_humidity: Some(Ratio(0.55)),
                            distance: None
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
                            relative_humidity: Some(Ratio(0.75)),
                            distance: None,
                        }
                    ]
                )
                .expect("Formatting should work")
            )
        );
    }
    #[test]
    fn format_temperature_and_distance() {
        assert_str_eq!(
            format!(
                r##"# HELP weather_temperature_celsius prometheus-weathermen temperature.
# TYPE weather_temperature_celsius gauge
# UNIT weather_temperature_celsius celsius
weather_temperature_celsius{{version="{0}",source="org.example",location="My Name",city="Some City",latitude="20.1000000",longitude="10.0123400"}} 25.5
# HELP weather_station_distance_meters prometheus-weathermen weather station distance in meters.
# TYPE weather_station_distance_meters gauge
# UNIT weather_station_distance_meters meters
weather_station_distance_meters{{version="{0}",source="org.example",location="My Name",city="Some City",latitude="20.1000000",longitude="10.0123400"}} 100.1
# EOF"##,
                crate::config::VERSION
            ),
            sort_output_deterministically(
                &format_metrics(
                    Format::Prometheus,
                    vec![Weather {
                        source: "org.example".into(),
                        coordinates: Coordinates {
                            latitude: Coordinate::from(20.1),
                            longitude: Coordinate::from(10.01234),
                        },
                        location: "My Name".into(),
                        city: "Some City".into(),
                        temperature: Celsius::from(25.5),
                        relative_humidity: None,
                        distance: Some(100.1.into())
                    }]
                )
                .expect("Formatting should work")
            )
        );
    }
}
