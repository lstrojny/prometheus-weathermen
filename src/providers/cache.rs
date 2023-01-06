use moka::sync::Cache;
use reqwest::blocking::Client;
use reqwest::{Method, Url};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::time::Duration;

#[derive(Deserialize, Debug, Clone)]
pub struct CacheConfiguration {
    #[serde(default = "default_refresh_interval")]
    #[serde(with = "humantime_serde")]
    pub refresh_interval: Duration,
}

fn default_refresh_interval() -> Duration {
    Duration::from_secs(60 * 10)
}

pub fn reqwest_cached_json<T: DeserializeOwned>(
    source: &str,
    cache: &Cache<String, String>,
    client: &Client,
    method: Method,
    url: Url,
) -> Result<T, String> {
    match reqwest_cached(source, cache, client, method, url) {
        Ok(result) => match serde_json::from_str::<T>(&result) {
            Ok(des) => Ok(des),
            Err(e) => Err(e.to_string()),
        },
        Err(e) => Err(e),
    }
}

pub fn reqwest_cached(
    source: &str,
    cache: &Cache<String, String>,
    client: &Client,
    method: Method,
    url: Url,
) -> Result<String, String> {
    let key = format!("{source} {method} {url}");
    let value = cache.get(&key);

    if let Some(value) = value {
        println!(
            "CACHED for {:?}",
            cache
                .policy()
                .time_to_live()
                .unwrap_or(Duration::from_secs(0))
        );
        return Ok(value);
    }

    let result = client.request(method, url).send();

    match result {
        Ok(response) => match response.text() {
            Ok(response) => {
                cache.insert(key, response.clone());

                Ok(response)
            }
            Err(err) => Err(err.to_string()),
        },
        Err(err) => Err(err.to_string()),
    }
}
