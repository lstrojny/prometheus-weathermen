use crate::providers::{Coordinates, Providers, WeatherProvider, WeatherRequest};
use anyhow::Context;
use itertools::Itertools;
use moka::sync::Cache;
use serde::Deserialize;
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;
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

fn parse() -> anyhow::Result<Config> {
    let config_files = [
        &format!("/etc/{NAME}/config.toml"),
        "config.toml",
        "config.toml.dist",
    ];
    let canonical_files = config_files.iter().filter_map(|p| path::absolute(p).ok());
    let config = canonical_files.clone().fold(None as Option<String>, {
        |accum, f| accum.or_else(|| fs::read_to_string(f).ok())
    });

    let contents = config.with_context(|| {
        format!(
            "Could not find config file. Tried these files: \"{}\"",
            canonical_files
                .map(|v| v.to_string_lossy().into_owned())
                .join("\", \"")
        )
    })?;

    let config = toml::from_str(&contents)?;

    Ok(config)
}

pub type ProviderTasks = Vec<(
    Arc<dyn WeatherProvider + Send + Sync>,
    WeatherRequest<Coordinates>,
    Cache<String, String>,
)>;

pub fn get_provider_tasks() -> anyhow::Result<ProviderTasks> {
    let config = parse()?;
    println!("Config dump {config:?}");

    let configured_providers = config
        .providers
        .with_context(|| "No providers configured")?;

    let mut tasks: ProviderTasks = vec![];

    for configured_provider in configured_providers.into_iter() {
        let cache = moka::sync::CacheBuilder::new(config.locations.len() as u64)
            .time_to_live(configured_provider.cache_lifetime())
            .build();
        println!("Found configured provider {configured_provider:?}");
        let locations = config.locations.clone();
        for (name, location) in locations {
            let configured_provider_for_task = configured_provider.clone();
            tasks.push((
                configured_provider_for_task,
                WeatherRequest {
                    name: location.name.unwrap_or(name),
                    query: location.coordinates,
                },
                cache.clone(),
            ));
        }
    }

    Ok(tasks)
}
