use log::debug;
use moka::sync::Cache;
use reqwest::blocking::Client;
use reqwest::{Method, Url};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Configuration {
    #[serde(default = "default_refresh_interval")]
    #[serde(with = "humantime_serde")]
    pub refresh_interval: Duration,
}

const fn default_refresh_interval() -> Duration {
    Duration::from_secs(60 * 10)
}

pub fn reqwest_cached_body_json<T: DeserializeOwned>(
    source: &str,
    cache: &Cache<String, String>,
    client: &Client,
    method: Method,
    url: Url,
) -> anyhow::Result<T> {
    let body = reqwest_cached_body(source, cache, client, method, url)?;
    let response = serde_json::from_str::<T>(&body)?;

    Ok(response)
}

pub fn reqwest_cached_body(
    source: &str,
    cache: &Cache<String, String>,
    client: &Client,
    method: Method,
    url: Url,
) -> anyhow::Result<String> {
    let key = format!("{source} {method} {url}");
    let value = cache.get(&key);

    debug!(
        "Checking cache item for request \"{method:#} {url:#}\" for {source:?} with lifetime {:?}",
        cache
            .policy()
            .time_to_live()
            .unwrap_or(Duration::from_secs(0))
    );

    if let Some(value) = value {
        debug!("Found cached item for \"{method:#} {url:#}\"");
        return Ok(value);
    }

    debug!("No cache item found for \"{method:#} {url:#}\". Requesting");

    let body = client.request(method, url).send()?.text()?;
    cache.insert(key, body.clone());

    Ok(body)
}
