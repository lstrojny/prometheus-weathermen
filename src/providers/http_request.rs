use crate::providers::HttpRequestCache;
use anyhow::anyhow;
use failsafe::backoff::{exponential, Exponential};
use failsafe::failure_policy::{consecutive_failures, ConsecutiveFailures};
use failsafe::{CircuitBreaker, Config, Error, StateMachine};
use log::{debug, trace};
use moka::sync::Cache as MokaCache;
use once_cell::sync::Lazy;
use reqwest::blocking::{Client, Response};
use reqwest::{Method, Url};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::RwLock;
use std::time::Duration;

pub type Cache = MokaCache<(Method, Url), String>;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Configuration {
    #[serde(default = "default_refresh_interval")]
    #[serde(with = "humantime_serde")]
    pub refresh_interval: Duration,
}

const fn default_refresh_interval() -> Duration {
    Duration::from_secs(60 * 10)
}

pub struct HttpCacheRequest<'a, R: Debug = String> {
    source: &'a str,
    client: &'a Client,
    cache: &'a HttpRequestCache,
    method: &'a Method,
    url: &'a Url,
    to_string: fn(response: Response) -> anyhow::Result<String>,
    deserialize: fn(string: &str) -> anyhow::Result<R>,
}

const CONSECUTIVE_FAILURE_COUNT: u32 = 3;
const EXPONENTIAL_BACKOFF_START_SECS: u64 = 30;
const EXPONENTIAL_BACKOFF_MAX_SECS: u64 = 300;

type HttpCircuitBreaker = StateMachine<ConsecutiveFailures<Exponential>, ()>;

static CIRCUIT_BREAKER_REGISTRY: Lazy<RwLock<HashMap<String, HttpCircuitBreaker>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

impl HttpCacheRequest<'_> {
    pub fn new<'a, T: Debug>(
        source: &'a str,
        client: &'a Client,
        cache: &'a HttpRequestCache,
        method: &'a Method,
        url: &'a Url,
        to_string: fn(response: Response) -> anyhow::Result<String>,
        deserialize: fn(string: &str) -> anyhow::Result<T>,
    ) -> HttpCacheRequest<'a, T> {
        HttpCacheRequest {
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
        cache: &'a HttpRequestCache,
        method: &'a Method,
        url: &'a Url,
    ) -> HttpCacheRequest<'a, T> {
        HttpCacheRequest::new::<T>(
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

pub fn request_cached<R: Debug>(request: &HttpCacheRequest<R>) -> anyhow::Result<R> {
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

    let cicruit_breaker_scope = request
        .url
        .host_str()
        .ok_or_else(|| anyhow!("Could not extract host from URL"))?;

    // Separate scope so read lock is dropped at the end if circuit breaker does not yet exist
    {
        let circuit_breaker_registry_ro = CIRCUIT_BREAKER_REGISTRY.read().expect("Poisoned lock");

        trace!("Read lock acquired for {:?}", cicruit_breaker_scope);

        if let Some(cb) = circuit_breaker_registry_ro.get(cicruit_breaker_scope) {
            return request_url_with_circuit_breaker(cicruit_breaker_scope, cb, request, &key);
        }

        drop(circuit_breaker_registry_ro);
    }

    // Separate scope so write lock is dropped at the end
    {
        trace!(
            "Trying to acquire write lock to instantiate circuit breaker {:?}",
            cicruit_breaker_scope
        );

        let mut circuit_breaker_registry_rw =
            CIRCUIT_BREAKER_REGISTRY.write().expect("Poisoned lock");
        trace!(
            "Write lock acquired to instantiate circuit breaker {:?}",
            cicruit_breaker_scope
        );

        if circuit_breaker_registry_rw.contains_key(cicruit_breaker_scope) {
            trace!(
                "Circuit breaker {:?} already instantiated, skipping",
                cicruit_breaker_scope
            );
        } else {
            trace!(
                "Circuit breaker {:?} not yet instantiated, instantiating",
                cicruit_breaker_scope
            );

            let circuit_breaker = Config::new()
                .failure_policy(consecutive_failures(
                    CONSECUTIVE_FAILURE_COUNT,
                    exponential(
                        Duration::from_secs(EXPONENTIAL_BACKOFF_START_SECS),
                        Duration::from_secs(EXPONENTIAL_BACKOFF_MAX_SECS),
                    ),
                ))
                .build();

            circuit_breaker_registry_rw.insert(cicruit_breaker_scope.to_string(), circuit_breaker);

            trace!("Circuit breaker {:?} instantiated", cicruit_breaker_scope);
        }

        drop(circuit_breaker_registry_rw);
    }

    trace!(
        "Trying to acquire read lock after circuit breaker {:?} was instantiated",
        cicruit_breaker_scope
    );
    let circuit_breaker_registry_ro = CIRCUIT_BREAKER_REGISTRY
        .read()
        .expect("Lock should not be poisoned");
    trace!(
        "Read lock acquired after circuit breaker {:?} was instantiated",
        cicruit_breaker_scope
    );
    let circuit_breaker = circuit_breaker_registry_ro
        .get(cicruit_breaker_scope)
        .expect("Circuit breaker must now exist");

    request_url_with_circuit_breaker(cicruit_breaker_scope, circuit_breaker, request, &key)
}

fn request_url_with_circuit_breaker<R: Debug>(
    circuit_breaker_scope: &str,
    circuit_breaker: &HttpCircuitBreaker,
    request: &HttpCacheRequest<R>,
    key: &(Method, Url),
) -> anyhow::Result<R> {
    match circuit_breaker.call(|| request_url(request)) {
        Err(Error::Inner(e)) => Err(anyhow!(e)),
        Err(Error::Rejected) => Err(anyhow!(
            "Circuit breaker {:?} is open and prevented request",
            circuit_breaker_scope
        )),
        Ok(response) => {
            trace!(
                "Request to {:?} return with status code {:?}",
                request.url.to_string(),
                response.status()
            );

            let body = (request.to_string)(response)?;

            request.cache.insert(key.clone(), body.clone());

            let des = (request.deserialize)(&body)?;

            Ok(des)
        }
    }
}

fn request_url<R: Debug>(request: &HttpCacheRequest<R>) -> anyhow::Result<Response> {
    let response = request
        .client
        .request(request.method.clone(), request.url.clone())
        .send()?;

    if !response.status().is_success() {
        return Err(anyhow!(
            "Request for provider {} return status code {}",
            request.source,
            response.status()
        ));
    }

    Ok(response)
}
