use crate::provider::units::Celsius;
use std::fmt::{Display, Formatter};

pub struct Coordinate(f32);
impl Coordinate {
    pub fn new(coordinate: f32) -> Self {
        Coordinate(coordinate)
    }
}
impl Clone for Coordinate {
    fn clone(&self) -> Self {
        Coordinate::new(self.0)
    }
}
impl Copy for Coordinate {}
impl Display for Coordinate {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.7}", self.0)
    }
}

pub struct Coordinates {
    latitude: Coordinate,
    longitude: Coordinate,
}
impl Coordinates {
    pub fn new(latitude: Coordinate, longitude: Coordinate) -> Self {
        Coordinates {
            latitude,
            longitude,
        }
    }
    pub fn get_latitude(&self) -> Coordinate {
        self.latitude
    }
    pub fn get_longitude(&self) -> Coordinate {
        self.longitude
    }
}
impl Clone for Coordinates {
    fn clone(&self) -> Self {
        Coordinates::new(self.latitude.clone(), self.longitude.clone())
    }
}
impl Copy for Coordinates {}

pub struct Weather {
    pub temperature: Celsius,
    pub coordinates: Coordinates,
}

pub trait WeatherProvider {
    fn for_coordinates(&self, coordinates: Coordinates) -> Weather;
}
