use crate::providers::HttpRequestBodyCache;
use anyhow::anyhow;
use failsafe::backoff::{exponential, Exponential};
use failsafe::failure_policy::{consecutive_failures, ConsecutiveFailures};
use failsafe::{CircuitBreaker, Config, Error, StateMachine};
use log::{debug, trace};
use moka::sync::Cache;
use reqwest::blocking::{Client, Response};
use reqwest::{Method, Url};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::time::Duration;

pub type HttpRequestBody = Cache<(Method, Url), String>;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Configuration {
    #[serde(default = "default_refresh_interval")]
    #[serde(with = "humantime_serde")]
    pub refresh_interval: Duration,
}

const fn default_refresh_interval() -> Duration {
    Duration::from_secs(60 * 10)
}

pub struct CachedHttpRequest<'a, R: Debug = String> {
    source: &'a str,
    client: &'a Client,
    cache: &'a HttpRequestBodyCache,
    method: &'a Method,
    url: &'a Url,
    to_string: fn(response: Response) -> anyhow::Result<String>,
    deserialize: fn(string: &str) -> anyhow::Result<R>,
}

const CONSECUTIVE_FAILURE_COUNT: u32 = 3;
const EXPONENTIAL_BACKOFF_START_SECS: u64 = 30;
const EXPONENTIAL_BACKOFF_MAX_SECS: u64 = 300;

lazy_static! {
    static ref CIRCUIT_BREAKER: StateMachine<ConsecutiveFailures<Exponential>, ()> = Config::new()
        .failure_policy(consecutive_failures(
            CONSECUTIVE_FAILURE_COUNT,
            exponential(
                Duration::from_secs(EXPONENTIAL_BACKOFF_START_SECS),
                Duration::from_secs(EXPONENTIAL_BACKOFF_MAX_SECS)
            )
        ))
        .build();
}

impl CachedHttpRequest<'_> {
    pub fn new<'a, T: Debug>(
        source: &'a str,
        client: &'a Client,
        cache: &'a HttpRequestBodyCache,
        method: &'a Method,
        url: &'a Url,
        to_string: fn(response: Response) -> anyhow::Result<String>,
        deserialize: fn(string: &str) -> anyhow::Result<T>,
    ) -> CachedHttpRequest<'a, T> {
        CachedHttpRequest {
            source,
            client,
            cache,
            method,
            url,
            to_string,
            deserialize,
        }
    }

    pub fn new_json_request<'a, T: Debug + DeserializeOwned>(
        source: &'a str,
        client: &'a Client,
        cache: &'a HttpRequestBodyCache,
        method: &'a Method,
        url: &'a Url,
    ) -> CachedHttpRequest<'a, T> {
        CachedHttpRequest::new::<T>(
            source,
            client,
            cache,
            method,
            url,
            response_to_string,
            serde_deserialize_body,
        )
    }
}

fn response_to_string(response: Response) -> anyhow::Result<String> {
    Ok(response.text()?)
}

fn serde_deserialize_body<T: Debug + DeserializeOwned>(body: &str) -> anyhow::Result<T> {
    trace!("Deserializing body {body:?}");
    Ok(serde_json::from_str(body)?)
}

pub fn reqwest_cached<R: Debug>(request: &CachedHttpRequest<R>) -> anyhow::Result<R> {
    let key = (request.method.clone(), request.url.clone());
    let value = request.cache.get(&key);

    debug!(
        "Checking cache item for request \"{:#} {:#}\" for {:?} with lifetime {:?}",
        request.method,
        request.url,
        request.source,
        request
            .cache
            .policy()
            .time_to_live()
            .unwrap_or(Duration::from_secs(0))
    );

    if let Some(value) = value {
        debug!(
            "Found cached item for \"{:#} {:#}\"",
            request.method, request.url
        );

        let des = (request.deserialize)(&value)?;

        return Ok(des);
    }

    debug!(
        "No cache item found for \"{:#} {:#}\". Requesting",
        request.method, request.url
    );

    match (*CIRCUIT_BREAKER).call(|| request_url(request)) {
        Err(Error::Inner(e)) => Err(anyhow!(e)),
        Err(Error::Rejected) => Err(anyhow!("Circuit breaker active and prevented request")),
        Ok(response) => {
            debug!("Status {}", response.status());

            let body = (request.to_string)(response)?;

            request.cache.insert(key, body.clone());

            let des = (request.deserialize)(&body)?;

            Ok(des)
        }
    }
}

fn request_url<R: Debug>(request: &CachedHttpRequest<R>) -> anyhow::Result<Response> {
    let response = request
        .client
        .request(request.method.clone(), request.url.clone())
        .send()?;

    if !response.status().is_success() {
        return Err(anyhow!("Status code {}", response.status()));
    }

    Ok(response)
}
