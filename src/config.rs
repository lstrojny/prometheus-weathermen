use crate::providers::{Coordinates, Providers};
use itertools::Itertools;
use serde::Deserialize;
use std::collections::HashMap;
use std::fmt::Debug;
use std::{fs, path};

pub const NAME: &str = env!("CARGO_PKG_NAME");
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Deserialize, Debug, Clone)]
pub struct Location {
    pub name: Option<String>,
    #[serde(flatten)]
    pub coordinates: Coordinates,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Config {
    #[serde(rename = "location")]
    pub locations: HashMap<String, Location>,
    #[serde(rename = "provider")]
    pub providers: Option<Providers>,
}

pub fn parse() -> Result<Config, toml::de::Error> {
    let config_files = [
        &format!("/etc/{NAME}/config.toml"),
        "config.toml",
        "config.toml.dist",
    ];
    let canonical_files = config_files.iter().filter_map(|p| path::absolute(p).ok());
    let config = canonical_files.clone().fold(None as Option<String>, {
        |accum, f| accum.or_else(|| fs::read_to_string(f).ok())
    });

    let contents = config.unwrap_or_else(|| {
        panic!(
            "Could not find config file. Tried these files: \"{}\"",
            canonical_files
                .map(|v| v.to_string_lossy().into_owned())
                .join("\", \"")
        )
    });
    toml::from_str(&contents)
}
