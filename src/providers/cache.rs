use crate::providers::HttpRequestBodyCache;
use anyhow::anyhow;
use log::{debug, trace};
use moka::sync::Cache;
use recloser::{Error, Recloser};
use reqwest::blocking::{Client, Response};
use reqwest::{Method, Url};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::{Arc, Mutex};
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

lazy_static! {
    static ref CIRCUIT_BREAKER_REGISTRY: Arc<Mutex<HashMap<String, Recloser>>> =
        Arc::new(Mutex::new(HashMap::new()));
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

    let hm_ref = Arc::clone(&CIRCUIT_BREAKER_REGISTRY);
    let cb_scope = request
        .url
        .host_str()
        .ok_or_else(|| anyhow!("Could not extract host from URL"))?;

    // Read lock must be dropped if circuit breaker does not yet exist
    {
        let cb_hm_r = hm_ref.lock().expect("Poisoned lock");

        trace!("Read lock acquired for {:?}", cb_scope);

        if cb_hm_r.contains_key(cb_scope) {
            let cb = cb_hm_r.get(cb_scope).expect("Checked before");

            return request_url_with_circuit_breaker(cb, request, &key);
        }
    }

    // Write lock needs to be dropped at the end of this scope
    {
        trace!("Trying to acquire write lock for {:?}", cb_scope);

        let mut cb_hm_rw = hm_ref.lock().expect("Poisoned lock");
        trace!("Write lock acquired for {:?}", cb_scope);

        if cb_hm_rw.contains_key(cb_scope) {
            trace!(
                "Circuit breaker already created for {:?}, skipping",
                cb_scope
            );
        } else {
            trace!(
                "Circuit breaker not yet created for {:?}, creating",
                cb_scope
            );

            let c = Recloser::custom()
                .error_rate(0.5)
                .closed_len(2)
                .half_open_len(1)
                .open_wait(Duration::from_secs(60))
                .build();

            cb_hm_rw.insert(cb_scope.to_string(), c);

            trace!("Circuit breaker created for {:?}", cb_scope);
        }
    }

    trace!(
        "Trying to acquire read lock after circuit breaker creation for {:?}",
        cb_scope
    );
    let cb_hm_r = hm_ref.lock().expect("Lock should not be poisoned");
    trace!(
        "Read lock acquired after circuit breaker creation for {:?}",
        cb_scope
    );
    let cb = cb_hm_r
        .get(cb_scope)
        .expect("Circuit breaker must now exist");

    request_url_with_circuit_breaker(cb, request, &key)
}

fn request_url_with_circuit_breaker<R: Debug>(
    cb: &Recloser,
    request: &CachedHttpRequest<R>,
    key: &(Method, Url),
) -> anyhow::Result<R> {
    match cb.call(|| request_url(request)) {
        Err(Error::Inner(e)) => Err(anyhow!(e)),
        Err(Error::Rejected) => Err(anyhow!("Circuit breaker active and prevented request")),
        Ok(response) => {
            debug!("Status {}", response.status());

            let body = (request.to_string)(response)?;

            request.cache.insert(key.clone(), body.clone());

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
        return Err(anyhow!(
            "Request for provider {} return status code {}",
            request.source,
            response.status()
        ));
    }

    Ok(response)
}
