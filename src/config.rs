use crate::providers::units::Coordinates;
use crate::providers::HttpRequestBodyCache;
use crate::providers::{Providers, WeatherProvider, WeatherRequest};
use anyhow::Context;
use figment::{
    providers::{Env, Format, Toml},
    Figment,
};
use log::{debug, info, warn, Level};
use rocket::config::Ident;
use rocket::figment::providers::Serialized;
use rocket::serde::Serialize;
use serde::Deserialize;
use std::collections::HashMap;
use std::fmt::Debug;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

pub const NAME: &str = env!("CARGO_PKG_NAME");
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const DEFAULT_CONFIG: &str = concat!("/etc/", env!("CARGO_PKG_NAME"), "/weathermen.toml");
const DEFAULT_PORT: u16 = 36333;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Location {
    pub name: Option<String>,
    #[serde(flatten)]
    pub coordinates: Coordinates,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    #[serde(rename = "location")]
    pub locations: HashMap<String, Location>,
    #[serde(rename = "provider")]
    pub providers: Option<Providers>,
    pub http: rocket::Config,
    pub auth: Option<CredentialsStore>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CredentialsStore(pub HashMap<String, String>);

impl<const N: usize> From<[(String, String); N]> for CredentialsStore {
    fn from(arr: [(String, String); N]) -> Self {
        Self(HashMap::from(arr))
    }
}

#[cfg(test)]
impl CredentialsStore {
    pub fn empty() -> Self {
        Self(HashMap::new())
    }
}

fn default_rocket_config() -> rocket::Config {
    rocket::Config {
        port: DEFAULT_PORT,
        ident: Ident::try_new(NAME.to_string()).unwrap(),
        ..rocket::Config::default()
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            locations: HashMap::new(),
            providers: None,
            http: default_rocket_config(),
            auth: None,
        }
    }
}

pub fn read(config_file: PathBuf, log_level: Level) -> anyhow::Result<Config> {
    info!("Reading config file {config_file:?}");

    let config: Config = Figment::new()
        .merge(Serialized::defaults(Config::default()))
        .merge(Toml::file(config_file))
        .merge((
            "http.log_level",
            match log_level {
                Level::Trace | Level::Debug => rocket::log::LogLevel::Debug,
                Level::Info | Level::Warn => rocket::log::LogLevel::Normal,
                Level::Error => rocket::log::LogLevel::Critical,
            },
        ))
        .merge(Env::prefixed("PROMW_").split("__"))
        .extract()?;

    debug!("Read config is {:?}", config);

    Ok(config)
}

pub type ProviderTasks = Vec<Task>;

#[derive(Clone)]
pub struct Task {
    pub provider: Arc<dyn WeatherProvider + Send + Sync>,
    pub request: WeatherRequest<Coordinates>,
    pub cache: HttpRequestBodyCache,
}

pub fn get_provider_tasks(config: Config) -> anyhow::Result<ProviderTasks> {
    let configured_providers = config
        .providers
        .with_context(|| "No providers configured")?;

    let mut tasks: ProviderTasks = vec![];

    for configured_provider in configured_providers {
        let max_capacity = config.locations.len() * configured_provider.cache_cardinality();
        let cache = moka::sync::CacheBuilder::new(max_capacity as u64)
            .time_to_live(configured_provider.refresh_interval())
            .build();

        debug!("Found configured provider {configured_provider:?}");

        if configured_provider.refresh_interval() < Duration::from_secs(60 * 5) {
            warn!(
                "Updating weather information more often than every 5 minutes is discouraged. Consider increasing the refresh interval for {:?}", 
                configured_provider.id()
            );
        }

        let locations = config.locations.clone();
        for (name, location) in locations {
            tasks.push(Task {
                provider: configured_provider.clone(),
                request: WeatherRequest {
                    name: location.name.unwrap_or(name),
                    query: location.coordinates,
                },
                cache: cache.clone(),
            });
        }
    }

    Ok(tasks)
}
