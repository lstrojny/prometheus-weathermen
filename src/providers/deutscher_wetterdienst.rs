use crate::providers::http_request::{request_cached, Configuration, HttpCacheRequest};
use crate::providers::units::{Celsius, Coordinate, Coordinates, Ratio};
use crate::providers::{
    calculate_distance, HttpRequestCache, Weather, WeatherProvider, WeatherRequest,
};
use anyhow::anyhow;
use chrono::Utc;
use const_format::concatcp;
use csv::Trim;
use geo::{Closest, ClosestPoint, MultiPoint, Point};
use log::{debug, trace};
use reqwest::blocking::Client;
use reqwest::{Method, Url};
use serde::{Deserialize, Serialize};
use std::io::{Cursor, Read};
use std::time::Duration;
use zip::ZipArchive;

const SOURCE_URI: &str = "de.dwd";
const BASE_URL: &str = "https://opendata.dwd.de/climate_environment/CDC/observations_germany/climate/10_minutes/air_temperature/now";
const STATION_LIST_URL: &str = concatcp!(BASE_URL, "/zehn_now_tu_Beschreibung_Stationen.txt");

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DeutscherWetterdienst {
    #[serde(flatten)]
    cache: Configuration,
}

#[derive(Deserialize, Debug, PartialEq, Clone)]
struct WeatherStation {
    #[serde(rename = "Stations_id")]
    station_id: String,
    #[serde(rename = "Stationsname")]
    name: String,
    #[serde(rename = "geoBreite")]
    latitude: Coordinate,
    #[serde(rename = "geoLaenge")]
    longitude: Coordinate,
}

fn strip_duplicate_spaces(data: &str) -> String {
    let mut prev_space = false;

    data.chars()
        .filter(|&c| {
            let cur_space = c == ' ';

            if cur_space && prev_space {
                return false;
            }

            prev_space = cur_space;

            true
        })
        .collect()
}

fn disambiguate_multi_words(data: &str) -> String {
    data.split('\n')
        .enumerate()
        .map(|(line_no, line)| {
            if line_no == 0 || line_no == 1 {
                line.to_owned()
            } else {
                disambiguate_multi_words_line(line)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[derive(Copy, Clone)]
enum Quote {
    None,
    Open,
    Closed,
}

fn disambiguate_multi_words_line(orig_line: &str) -> String {
    let mut orig_line_reversed = orig_line.trim().chars().rev().peekable();
    let mut line = String::new();
    let mut quote = Quote::None;
    while let Some(cur_char) = orig_line_reversed.next() {
        match (cur_char, orig_line_reversed.peek(), quote) {
            (' ', Some(&next_char), Quote::None) if next_char != ' ' => {
                line.push_str(" \"");
                quote = Quote::Open;
            }
            (' ', Some(&next_char), Quote::Open) if next_char.is_ascii_digit() => {
                line.push('"');
                line.push(cur_char);
                quote = Quote::Closed;
            }
            _ => line.push(cur_char),
        }
    }
    line.chars().rev().collect()
}

fn parse_weather_station_list_csv(data: &str) -> anyhow::Result<Vec<WeatherStation>> {
    let stripped = strip_duplicate_spaces(data);
    let processed = disambiguate_multi_words(&stripped);

    let reader = csv::ReaderBuilder::new()
        .delimiter(b' ')
        .comment(Some(b'-'))
        .trim(Trim::All)
        .flexible(true)
        .from_reader(processed.as_bytes());

    Ok(reader
        .into_deserialize::<WeatherStation>()
        .collect::<Result<_, _>>()?)
}

fn find_closest_weather_station<'stations>(
    coords: &Coordinates,
    weather_stations: &'stations [WeatherStation],
) -> anyhow::Result<&'stations WeatherStation> {
    let point: Point<f64> = Point::new(
        coords.longitude.clone().into(),
        coords.latitude.clone().into(),
    );
    let points = MultiPoint::new(
        weather_stations
            .iter()
            .map(|s| Point::new(s.longitude.clone().into(), s.latitude.clone().into()))
            .collect(),
    );

    match points.closest_point(&point) {
        Closest::SinglePoint(closest_point) | Closest::Intersection(closest_point) => {
            let matching_station = weather_stations
                .iter()
                .find(|station| {
                    station.longitude == closest_point.x().into()
                        && station.latitude == closest_point.y().into()
                })
                .ok_or_else(|| anyhow!("Could not find matching station"))?;

            Ok(matching_station)
        }
        Closest::Indeterminate => Err(anyhow!("Could not find closest point")),
    }
}

#[allow(clippy::case_sensitive_file_extension_comparisons)]
fn is_measurement_file(file_name: &str) -> bool {
    file_name.starts_with("produkt_zehn_now") && file_name.ends_with(".txt")
}

fn read_measurement_data_zip(buf: &[u8]) -> anyhow::Result<String> {
    let reader = Cursor::new(buf);
    let mut zip = ZipArchive::new(reader)?;

    for i in 0..zip.len() {
        let mut file = zip.by_index(i)?;

        if !is_measurement_file(file.name()) {
            trace!("Skipping file in measurement data zip: {}", file.name());
            continue;
        }

        debug!(
            "Found matching file in measurement data zip: {}",
            file.name()
        );

        let mut str_buf = String::new();
        file.read_to_string(&mut str_buf)?;

        return Ok(str_buf);
    }

    Err(anyhow!("Could not find weather data file in ZIP archive"))
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
struct Measurement {
    #[serde(rename = "STATIONS_ID")]
    _station_id: String,
    #[serde(rename = "MESS_DATUM", with = "minute_precision_date_format")]
    time: chrono::DateTime<Utc>,
    #[serde(rename = "PP_10")]
    _atmospheric_pressure: String,
    #[serde(rename = "TT_10")]
    temperature_200_centimers: Celsius,
    #[serde(rename = "TM5_10")]
    _temperature_5_centimeters: Celsius,
    #[serde(rename = "RF_10")]
    relative_humidity_200_centimeters: Ratio,
    #[serde(rename = "TD_10")]
    _dew_point_temperature_200_centimeters: Celsius,
}

mod minute_precision_date_format {
    use chrono::{DateTime, NaiveDateTime, Utc};
    use serde::de::Error;
    use serde::{self, Deserialize, Deserializer};

    const FORMAT: &str = "%Y%m%d%H%M";

    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        NaiveDateTime::parse_from_str(&s, FORMAT)
            .map(|v| v.and_utc())
            .map_err(Error::custom)
    }
}

fn parse_measurement_data_csv(data: &String) -> anyhow::Result<Vec<Measurement>> {
    let reader = csv::ReaderBuilder::new()
        .delimiter(b';')
        .double_quote(false)
        .trim(Trim::All)
        .from_reader(data.as_bytes());

    Ok(reader
        .into_deserialize::<Measurement>()
        .collect::<Result<_, _>>()?)
}

fn reqwest_cached_measurement_csv(
    cache: &HttpRequestCache,
    client: &Client,
    station_id: &String,
) -> anyhow::Result<String> {
    let method = Method::GET;
    let url = Url::parse(&format!(
        "{BASE_URL}/10minutenwerte_TU_{station_id}_now.zip"
    ))?;

    request_cached(&HttpCacheRequest::new(
        SOURCE_URI,
        client,
        cache,
        &method,
        &url,
        |body| read_measurement_data_zip(body),
    ))
}

impl WeatherProvider for DeutscherWetterdienst {
    fn id(&self) -> &str {
        SOURCE_URI
    }

    fn for_coordinates(
        &self,
        client: &Client,
        cache: &HttpRequestCache,
        request: &WeatherRequest<Coordinates>,
    ) -> anyhow::Result<Weather> {
        let stations = request_cached(&HttpCacheRequest::new(
            SOURCE_URI,
            client,
            cache,
            &Method::GET,
            &Url::parse(STATION_LIST_URL)?,
            |body| {
                let str: String = body
                    .iter()
                    .filter_map(|&c| char::from_u32(c.into()))
                    .collect();

                parse_weather_station_list_csv(&str)
            },
        ))?;

        let closest_station = find_closest_weather_station(&request.query, &stations)?;
        trace!("Found closest weather station {:?}", closest_station);
        let measurement_csv =
            reqwest_cached_measurement_csv(cache, client, &closest_station.station_id)?;
        let measurements = parse_measurement_data_csv(&measurement_csv)?;

        match &*measurements {
            [.., latest_measurement] => {
                debug!(
                    "Using latest measurement from {}: {:?}",
                    latest_measurement.time,
                    latest_measurement.clone()
                );

                let coordinates = Coordinates {
                    latitude: closest_station.latitude.clone(),
                    longitude: closest_station.longitude.clone(),
                };

                let distance = calculate_distance(&request.query, &coordinates);

                Ok(Weather {
                    source: SOURCE_URI.into(),
                    location: request.name.clone(),
                    city: Some(closest_station.name.clone()),
                    coordinates,
                    distance: Some(distance),
                    temperature: latest_measurement.temperature_200_centimers,
                    relative_humidity: Some(latest_measurement.relative_humidity_200_centimeters),
                })
            }
            [] => Err(anyhow!("Empty measurement list")),
        }
    }

    fn refresh_interval(&self) -> Duration {
        self.cache.refresh_interval
    }

    fn cache_cardinality(&self) -> usize {
        2
    }
}

#[cfg(test)]
mod tests {
    mod parse_weather_station_list {
        use crate::providers::deutscher_wetterdienst::{
            parse_weather_station_list_csv, WeatherStation,
        };
        use pretty_assertions::assert_eq;

        #[test]
        fn parse_short_list() {
            assert_eq!(
                parse_weather_station_list_csv("Stations_id von_datum bis_datum Stationshoehe geoBreite geoLaenge Stationsname Bundesland\n\
----------- --------- --------- ------------- --------- --------- ----------------------------------------- ----------\n\
00044 20070209 20230111             44     52.7553    7.4815 Gro\u{df} Ber\u{df}en                             Niedersachsen                                                                                     \n\
").expect("Parsing works"),
                vec![WeatherStation {
                    station_id: "00044".into(),
                    name: "Gro\u{df} Ber\u{df}en".into(),
                    latitude: 52.7553_f64.into(),
                    longitude: 7.4815_f64.into(),
                }]
            );
        }

        #[test]
        fn parse_error() {
            assert!(
                parse_weather_station_list_csv("Stations_id von_datum bis_datum Stationshoehe geoBreite geoLaenge Stationsname Bundesland\n\
----------- --------- --------- ------------- --------- --------- ----------------------------------------- ----------\n\
broken\n\
").expect_err("Will fail to parse").to_string().contains("CSV deserialize error: record 1"),
            );
        }
    }

    mod find_closes_weather_station {
        use crate::providers::deutscher_wetterdienst::{
            find_closest_weather_station, WeatherStation,
        };
        use crate::providers::units::Coordinates;
        use pretty_assertions::assert_eq;

        #[test]
        fn find_closest_station_to_a_coordinate() {
            assert_eq!(
                find_closest_weather_station(
                    &Coordinates {
                        latitude: 48.11591_f64.into(),
                        longitude: 11.570_906_f64.into(),
                    },
                    &[
                        WeatherStation {
                            station_id: "03379".into(),
                            name: "M\u{fc}nchen-Stadt".into(),
                            latitude: 48.1632_f64.into(),
                            longitude: 11.5429_f64.into(),
                        },
                        WeatherStation {
                            station_id: "01262".into(),
                            name: "M\u{fc}nchen-Flughafen".into(),
                            latitude: 48.3477_f64.into(),
                            longitude: 11.8134_f64.into(),
                        },
                    ]
                )
                .expect("Should find something"),
                &WeatherStation {
                    station_id: "03379".into(),
                    name: "M\u{fc}nchen-Stadt".into(),
                    latitude: 48.1632_f64.into(),
                    longitude: 11.5429_f64.into(),
                }
            );
        }
    }

    mod strip_duplicate_spaces {
        use crate::providers::deutscher_wetterdienst::strip_duplicate_spaces;
        use pretty_assertions::assert_str_eq;

        #[test]
        fn not_stripped_if_not_needed() {
            assert_str_eq!(strip_duplicate_spaces("foo bar"), "foo bar");
        }

        #[test]
        fn strips_two_spaces() {
            assert_str_eq!(strip_duplicate_spaces("foo  bar"), "foo bar");
        }

        #[test]
        fn strips_more_than_two_spaces() {
            assert_str_eq!(strip_duplicate_spaces("foo   bar"), "foo bar");
        }

        #[test]
        fn strips_multiple_occurrences() {
            assert_str_eq!(strip_duplicate_spaces("foo   bar   baz "), "foo bar baz ");
        }
    }

    mod parse_measurement_data_csv {
        use crate::providers::deutscher_wetterdienst::{parse_measurement_data_csv, Measurement};
        use crate::providers::units::Ratio;
        use chrono::{DateTime, Utc};
        use pretty_assertions::assert_eq;

        #[test]
        fn parse_example() {
            assert_eq!(
                &*parse_measurement_data_csv(
                    &"STATIONS_ID;MESS_DATUM;  QN;PP_10;TT_10;TM5_10;RF_10;TD_10;eor\n\
            379;202301120000;    2;   -999;   5.1;   2.5;  82.6;   2.4;eor"
                        .to_owned(),
                )
                .expect("Parsing works"),
                [Measurement {
                    _station_id: "379".into(),
                    _atmospheric_pressure: "-999".into(),
                    _dew_point_temperature_200_centimeters: 2.4.into(),
                    _temperature_5_centimeters: 2.5.into(),
                    time: DateTime::parse_from_rfc3339("2023-01-12T00:00:00Z")
                        .expect("Static value")
                        .with_timezone(&Utc {}),
                    temperature_200_centimers: 5.1.into(),
                    relative_humidity_200_centimeters: Ratio::Percentage(82.6),
                }]
            );
        }
    }
}
