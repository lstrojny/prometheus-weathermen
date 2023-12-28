use crate::providers::units::Coordinates;
use crate::providers::HttpRequestCache;
use crate::providers::{Providers, WeatherProvider, WeatherRequest};
use anyhow::Context;
use const_format::concatcp;
use derive_more::{Display, From, Into};
use figment::{
    providers::{Env, Format, Toml},
    Figment,
};
use log::{debug, info, warn, Level};
use once_cell::sync::OnceCell;
use reqwest::blocking::Client;
use rocket::config::Ident;
use rocket::figment::providers::Serialized;
use rocket::serde::Serialize;
use serde::Deserialize;
use std::collections::hash_map::Iter;
use std::collections::HashMap;
use std::fmt::Debug;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

pub const NAME: &str = env!("CARGO_PKG_NAME");
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const DEFAULT_CONFIG: &str = concatcp!("/etc/", NAME, "/weathermen.toml");
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

#[derive(Serialize, Deserialize, Debug, Into, Clone, Display, From)]
pub struct Hash(String);

#[derive(Serialize, Deserialize, Debug, From, Clone, Default)]
pub struct CredentialsStore(HashMap<String, Hash>);

impl CredentialsStore {
    pub fn iter(&self) -> Iter<String, Hash> {
        self.0.iter()
    }
}

impl<const N: usize> From<[(String, Hash); N]> for CredentialsStore {
    fn from(arr: [(String, Hash); N]) -> Self {
        Self(HashMap::from(arr))
    }
}

const BCRYPT_DEFAULT_PASSWORD: &str = "fakepassword";
const BCRYPT_DEFAULT_COST: u32 = bcrypt::DEFAULT_COST;

static BCRYPT_DEFAULT_HASH: OnceCell<Hash> = OnceCell::new();

impl CredentialsStore {
    pub fn default_hash(&self) -> &Hash {
        BCRYPT_DEFAULT_HASH.get_or_init(|| Self::hash_default_password(self.max_cost()))
    }

    fn hash_default_password(cost: Option<u32>) -> Hash {
        bcrypt::hash(BCRYPT_DEFAULT_PASSWORD, cost.unwrap_or(BCRYPT_DEFAULT_COST))
            .ok()
            .map_or_else(
                || Self::hash_default_password(Some(BCRYPT_DEFAULT_COST)),
                Into::into,
            )
    }

    fn max_cost(&self) -> Option<u32> {
        self.0.values().map(Self::cost).max().flatten()
    }

    fn cost(hash: &Hash) -> Option<u32> {
        hash.to_string()
            .split('$')
            .nth(2)
            .and_then(|v| v.parse::<u32>().ok())
    }
}

fn default_rocket_config() -> rocket::Config {
    rocket::Config {
        port: DEFAULT_PORT,
        ident: Ident::try_new(NAME.to_string()).expect("Hardcoded, cannot fail"),
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
    pub client: Client,
    pub cache: HttpRequestCache,
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
                client: Client::new(),
                cache: cache.clone(),
            });
        }
    }

    Ok(tasks)
}

#[cfg(test)]
mod tests {
    mod default_hash {
        use crate::config::{CredentialsStore, BCRYPT_DEFAULT_COST};

        #[test]
        fn none_if_empty_string() {
            assert_eq!(CredentialsStore::cost(&"".to_string().into()), None);
        }

        #[test]
        fn none_if_unparseable_string() {
            assert_eq!(CredentialsStore::cost(&"$12".to_string().into()), None);
        }

        #[test]
        fn none_if_incomplete_string() {
            assert_eq!(CredentialsStore::cost(&"$2a$".to_string().into()), None);
        }

        #[test]
        fn cost_128() {
            assert_eq!(
                CredentialsStore::cost(
                    &"$2a$255$R9h/cIPz0gi.URNNX3kh2OPST9/PgBkqquzi.Ss7KIUgO2t0jWMUW"
                        .to_string()
                        .into()
                ),
                Some(255u32)
            );
        }

        #[test]
        fn cost_10() {
            assert_eq!(
                CredentialsStore::cost(
                    &"$2a$10$R9h/cIPz0gi.URNNX3kh2OPST9/PgBkqquzi.Ss7KIUgO2t0jWMUW"
                        .to_string()
                        .into()
                ),
                Some(10u32)
            );
        }

        #[test]
        fn cost_5() {
            assert_eq!(
                CredentialsStore::cost(
                    &"$2a$05$R9h/cIPz0gi.URNNX3kh2OPST9/PgBkqquzi.Ss7KIUgO2t0jWMUW"
                        .to_string()
                        .into()
                ),
                Some(5u32)
            );
        }

        #[test]
        fn cost_5_unpadded() {
            assert_eq!(
                CredentialsStore::cost(
                    &"$2a$5$R9h/cIPz0gi.URNNX3kh2OPST9/PgBkqquzi.Ss7KIUgO2t0jWMUW"
                        .to_string()
                        .into()
                ),
                Some(5u32)
            );
        }

        #[test]
        fn default_hash_with_cost_too_low() {
            assert_default_hash_with_cost(Some(0), BCRYPT_DEFAULT_COST);
        }

        #[test]
        fn default_hash_with_cost_too_high() {
            assert_default_hash_with_cost(Some(255), BCRYPT_DEFAULT_COST);
        }

        #[test]
        fn default_hash_with_no_cost() {
            assert_default_hash_with_cost(None, BCRYPT_DEFAULT_COST);
        }

        #[test]
        fn default_hash_with_cost_ok() {
            assert_default_hash_with_cost(Some(5), 5);
        }

        fn assert_default_hash_with_cost(given_cost: Option<u32>, expected_cost: u32) {
            assert_eq!(
                CredentialsStore::hash_default_password(given_cost)
                    .to_string()
                    .starts_with(format!("$2b${:02}", expected_cost).as_str()),
                true
            );
        }
    }
}
