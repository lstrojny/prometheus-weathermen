use crate::provider::provider::{Coordinates, Provider};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;

#[derive(Deserialize, Debug, Clone)]
pub struct Location {
    pub name: Option<String>,
    #[serde(flatten)]
    pub coordinates: Coordinates,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Config {
    pub location: HashMap<String, Location>,
    pub provider: Option<Provider>,
}

pub fn parse() -> Result<Config, toml::de::Error> {
    let contents = fs::read_to_string("config.toml.dist").expect("Read error");
    toml::from_str(&*contents)
}
