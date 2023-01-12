use crate::providers::cache::{reqwest_cached_body, Configuration};
use crate::providers::units::{Celsius, Coordinate, Coordinates, Ratio};
use crate::providers::{HttpRequestBodyCache, Weather, WeatherProvider, WeatherRequest};
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
    pub cache: Configuration,
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
    let mut in_space = false;

    data.chars()
        .filter(|c| {
            if *c == ' ' {
                if in_space {
                    return false;
                }
                in_space = true;
            } else {
                in_space = false;
            }

            true
        })
        .collect()
}

fn parse_weather_station_list_csv(data: &str) -> Vec<WeatherStation> {
    let stripped = strip_duplicate_spaces(data);

    let reader = csv::ReaderBuilder::new()
        .delimiter(b' ')
        .double_quote(false)
        .comment(Some(b'-'))
        .trim(Trim::All)
        .flexible(true)
        .from_reader(stripped.as_bytes());

    reader
        .into_deserialize::<WeatherStation>()
        .map(|m| m.expect("Should always succeed"))
        .collect::<Vec<WeatherStation>>()
}

fn find_closest_weather_station<'a>(
    coords: &Coordinates,
    weather_stations: &'a [WeatherStation],
) -> anyhow::Result<&'a WeatherStation> {
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
        Closest::SinglePoint(point) | Closest::Intersection(point) => {
            let matching_station = weather_stations
                .iter()
                .find(|s| s.longitude == point.x().into() && s.latitude == point.y().into())
                .expect("Must be able to find matching weather station");

            Ok(matching_station)
        }
        Closest::Indeterminate => Err(anyhow!("Could not find closest point")),
    }
}

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

        let mut buf: String = String::new();
        file.read_to_string(&mut buf)?;

        return Ok(buf);
    }

    Err(anyhow!("Could not find weather data file in ZIP archive"))
}

#[derive(Deserialize, Debug, Clone)]
struct Measurement {
    #[serde(rename = "STATIONS_ID")]
    _station_id: String,
    #[serde(rename = "MESS_DATUM", with = "minute_precision_date_format")]
    _time: chrono::DateTime<Utc>,
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
    use chrono::{DateTime, TimeZone, Utc};
    use serde::{self, Deserialize, Deserializer};

    const FORMAT: &str = "%Y%m%d%H%M";

    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Utc.datetime_from_str(&s, FORMAT)
            .map_err(serde::de::Error::custom)
    }
}

fn parse_measurement_data_csv(data: &String) -> Vec<Measurement> {
    let reader = csv::ReaderBuilder::new()
        .delimiter(b';')
        .double_quote(false)
        .trim(Trim::All)
        .from_reader(data.as_bytes());

    reader
        .into_deserialize::<Measurement>()
        .map(|m| m.expect("Should always succeed"))
        .collect::<Vec<Measurement>>()
}

fn reqwest_cached_measurement_csv(
    cache: &HttpRequestBodyCache,
    client: &Client,
    station_id: &String,
) -> anyhow::Result<String> {
    let method = Method::GET;
    let url = Url::parse(&format!(
        "{BASE_URL}/10minutenwerte_TU_{station_id}_now.zip"
    ))?;

    let key = (method.clone(), url.clone());
    let value = cache.get(&key);

    if let Some(csv) = value {
        debug!("Found cached measurement data for {}", station_id);
        return Ok(csv);
    }

    debug!("No cached measurement data found for {}", station_id);

    let zip = client.request(method, url).send()?.bytes();
    let csv = read_measurement_data_zip(&zip?)?;

    cache.insert(key, csv.clone());

    Ok(csv)
}

impl WeatherProvider for DeutscherWetterdienst {
    fn id(&self) -> &str {
        SOURCE_URI
    }

    fn for_coordinates(
        &self,
        cache: &HttpRequestBodyCache,
        request: &WeatherRequest<Coordinates>,
    ) -> anyhow::Result<Weather> {
        let client = Client::new();

        let station_csv = reqwest_cached_body(
            SOURCE_URI,
            cache,
            &client,
            Method::GET,
            Url::parse(STATION_LIST_URL)?,
            Some("iso-8859-15"),
        )?;

        let stations = parse_weather_station_list_csv(&station_csv);
        let closest_station = find_closest_weather_station(&request.query, &stations)?;
        let measurement_csv =
            reqwest_cached_measurement_csv(cache, &client, &closest_station.station_id)?;
        let measurements = parse_measurement_data_csv(&measurement_csv);
        let measurement = measurements.last().expect("Taking last measurement info");

        debug!("Found last measurement: {:?}", measurement.clone());

        Ok(Weather {
            source: SOURCE_URI.into(),
            location: request.name.clone(),
            city: closest_station.name.clone(),
            coordinates: Coordinates {
                latitude: closest_station.latitude.clone(),
                longitude: closest_station.longitude.clone(),
            },
            temperature: measurement.temperature_200_centimers,
            relative_humidity: Some(measurement.relative_humidity_200_centimeters),
        })
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
    use crate::providers::deutscher_wetterdienst::{
        find_closest_weather_station, parse_weather_station_list_csv, WeatherStation,
    };
    use crate::providers::units::Coordinates;

    #[test]
    fn parse_csv() -> () {
        assert_eq!(vec![WeatherStation {
            station_id: "00044".into(),
            name: "Großenkneten".into(),
            latitude: 52.9336.into(),
            longitude: 8.2370.into(),
        }], parse_weather_station_list_csv(&"Stations_id von_datum bis_datum Stationshoehe geoBreite geoLaenge Stationsname Bundesland\n\
----------- --------- --------- ------------- --------- --------- ----------------------------------------- ----------\n\
00044 20070209 20230111             44     52.9336    8.2370 Großenkneten                             Niedersachsen                                                                                     \n\
".to_string()));
    }

    #[test]
    fn find_closest() -> () {
        assert_eq!(
            &WeatherStation {
                station_id: "03379".into(),
                name: "München-Stadt".into(),
                latitude: 48.1632.into(),
                longitude: 11.5429.into(),
            },
            find_closest_weather_station(
                &Coordinates {
                    latitude: 48.11591.into(),
                    longitude: 11.570906.into(),
                },
                &vec![
                    WeatherStation {
                        station_id: "03379".into(),
                        name: "München-Stadt".into(),
                        latitude: 48.1632.into(),
                        longitude: 11.5429.into(),
                    },
                    WeatherStation {
                        station_id: "01262".into(),
                        name: "München-Flughafen".into(),
                        latitude: 48.3477.into(),
                        longitude: 11.8134.into(),
                    },
                ]
            )
            .expect("Should find something")
        );
    }
}
