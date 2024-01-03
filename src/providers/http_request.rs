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

pub type Cache = MokaCache<(Method, Url), Vec<u8>>;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Configuration {
    #[serde(default = "default_refresh_interval")]
    #[serde(with = "humantime_serde")]
    pub(crate) refresh_interval: Duration,
}

const fn default_refresh_interval() -> Duration {
    Duration::from_secs(60 * 10)
}

pub struct HttpCacheRequest<'req, R: Debug = String> {
    source: &'req str,
    client: &'req Client,
    cache: &'req HttpRequestCache,
    method: &'req Method,
    url: &'req Url,
    deserialize: fn(body: &Vec<u8>) -> anyhow::Result<R>,
}

const CONSECUTIVE_FAILURE_COUNT: u32 = 3;
const EXPONENTIAL_BACKOFF_START_SECS: u64 = 30;
const EXPONENTIAL_BACKOFF_MAX_SECS: u64 = 300;

type HttpCircuitBreaker = StateMachine<ConsecutiveFailures<Exponential>, ()>;

static CIRCUIT_BREAKER_REGISTRY: Lazy<RwLock<HashMap<String, HttpCircuitBreaker>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

impl HttpCacheRequest<'_> {
    pub(crate) fn new<'req, T: Debug>(
        source: &'req str,
        client: &'req Client,
        cache: &'req HttpRequestCache,
        method: &'req Method,
        url: &'req Url,
        deserialize: fn(body: &Vec<u8>) -> anyhow::Result<T>,
    ) -> HttpCacheRequest<'req, T> {
        HttpCacheRequest {
            source,
            client,
            cache,
            method,
            url,
            deserialize,
        }
    }

    pub fn new_json_request<'req, T: Debug + DeserializeOwned>(
        source: &'req str,
        client: &'req Client,
        cache: &'req HttpRequestCache,
        method: &'req Method,
        url: &'req Url,
    ) -> HttpCacheRequest<'req, T> {
        HttpCacheRequest::new::<T>(source, client, cache, method, url, serde_deserialize_body)
    }
}

fn serde_deserialize_body<T: Debug + DeserializeOwned>(body: &Vec<u8>) -> anyhow::Result<T> {
    trace!("Deserializing body {body:?}");
    Ok(serde_json::from_slice(body)?)
}

pub(in crate::providers) fn request_cached<R: Debug>(
    request: &HttpCacheRequest<R>,
) -> anyhow::Result<R> {
    let key = (request.method.clone(), request.url.clone());

    let value = request.cache.try_get_with_by_ref(&key, || {
        debug!(
            "Generating cache item for request \"{:#} {:#}\" for {} with lifetime {:?}",
            request.method,
            request.url,
            request.source,
            request
                .cache
                .policy()
                .time_to_live()
                .unwrap_or(Duration::from_secs(0))
        );

        let circuit_breaker_scope = request
            .url
            .host_str()
            .ok_or_else(|| anyhow!("Could not extract host from URL"))?;

        // Separate scope so read lock is dropped at the end if circuit breaker does not yet exist
        {
            let circuit_breaker_registry_ro =
                CIRCUIT_BREAKER_REGISTRY.read().expect("Poisoned lock");

            trace!("Read lock acquired for {}", circuit_breaker_scope);

            if let Some(cb) = circuit_breaker_registry_ro.get(circuit_breaker_scope) {
                return request_url_with_circuit_breaker(circuit_breaker_scope, cb, request);
            }

            drop(circuit_breaker_registry_ro);
        };

        ensure_circuit_breaker(circuit_breaker_scope);

        trace!(
            "Trying to acquire read lock after circuit breaker {} was instantiated",
            circuit_breaker_scope
        );
        CIRCUIT_BREAKER_REGISTRY
            .read()
            .map_err(|e| anyhow!("Circuit breaker RO lock is poisoned: {}", e.to_string()))
            .and_then(|circuit_breaker_registry_ro| {
                trace!(
                    "Read lock acquired after circuit breaker {} was instantiated",
                    circuit_breaker_scope
                );
                circuit_breaker_registry_ro
                    .get(circuit_breaker_scope)
                    .map_or_else(
                        || Err(anyhow!("Circuit breaker not found")),
                        |circuit_breaker| {
                            request_url_with_circuit_breaker(
                                circuit_breaker_scope,
                                circuit_breaker,
                                request,
                            )
                        },
                    )
            })
    });

    match value {
        Ok(v) => Ok((request.deserialize)(&v)?),
        Err(e) => Err(anyhow!(e)),
    }
}

fn ensure_circuit_breaker(circuit_breaker_scope: &str) {
    trace!(
        "Trying to acquire write lock to instantiate circuit breaker {}",
        circuit_breaker_scope
    );

    let mut circuit_breaker_registry_rw = CIRCUIT_BREAKER_REGISTRY.write().expect("Poisoned lock");
    trace!(
        "Write lock acquired to instantiate circuit breaker {}",
        circuit_breaker_scope
    );

    if !circuit_breaker_registry_rw.contains_key(circuit_breaker_scope) {
        trace!(
            "Circuit breaker {} not yet instantiated, instantiating",
            circuit_breaker_scope
        );

        let circuit_breaker = create_circuit_breaker();

        circuit_breaker_registry_rw.insert(circuit_breaker_scope.to_owned(), circuit_breaker);
        drop(circuit_breaker_registry_rw);

        trace!("Circuit breaker {} instantiated", circuit_breaker_scope);
    }
}

fn create_circuit_breaker() -> StateMachine<ConsecutiveFailures<Exponential>, ()> {
    Config::new()
        .failure_policy(consecutive_failures(
            CONSECUTIVE_FAILURE_COUNT,
            exponential(
                Duration::from_secs(EXPONENTIAL_BACKOFF_START_SECS),
                Duration::from_secs(EXPONENTIAL_BACKOFF_MAX_SECS),
            ),
        ))
        .build()
}

fn request_url_with_circuit_breaker<R: Debug>(
    circuit_breaker_scope: &str,
    circuit_breaker: &HttpCircuitBreaker,
    request: &HttpCacheRequest<R>,
) -> anyhow::Result<Vec<u8>> {
    match circuit_breaker.call(|| request_url(request)) {
        Err(Error::Inner(e)) => Err(anyhow!(e)),
        Err(Error::Rejected) => Err(anyhow!(
            "Circuit breaker {} is open and prevented request",
            circuit_breaker_scope
        )),
        Ok(response) => {
            trace!(
                "Request to {} return with status code {}",
                request.url.to_string(),
                response.status()
            );

            Ok(response.bytes().map(|v| v.to_vec())?)
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
