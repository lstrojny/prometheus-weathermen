use crate::providers::HttpRequestBodyCache;
use log::debug;
use moka::sync::Cache;
use reqwest::blocking::{Client, Response};
use reqwest::{Method, Url};
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

pub struct Request<'a, R: Debug = String> {
    pub source: &'a str,
    pub cache: &'a HttpRequestBodyCache,
    pub client: &'a Client,
    pub method: &'a Method,
    pub url: &'a Url,
    pub to_string: fn(response: Response) -> anyhow::Result<String>,
    pub deserialize: fn(string: &str) -> anyhow::Result<R>,
}

impl Request<'_> {
    pub fn new<'a, T: Debug>(
        source: &'a str,
        cache: &'a HttpRequestBodyCache,
        client: &'a Client,
        method: &'a Method,
        url: &'a Url,
        to_string: fn(response: Response) -> anyhow::Result<String>,
        deserialize: fn(string: &str) -> anyhow::Result<T>,
    ) -> Request<'a, T> {
        Request {
            source,
            url,
            method,
            cache,
            client,
            to_string,
            deserialize,
        }
    }

    pub fn new_json_request<'a, T: Debug + for<'b> serde::Deserialize<'b>>(
        source: &'a str,
        cache: &'a HttpRequestBodyCache,
        client: &'a Client,
        method: &'a Method,
        url: &'a Url,
    ) -> Request<'a, T> {
        Request::new::<T>(
            source,
            cache,
            client,
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

fn serde_deserialize_body<T: Debug + for<'a> serde::Deserialize<'a>>(
    body: &str,
) -> anyhow::Result<T> {
    Ok(serde_json::from_str(&body)?)
}

pub fn reqwest_cached<R: Debug>(request: Request<R>) -> anyhow::Result<R> {
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

    let response = request
        .client
        .request(request.method.clone(), request.url.clone())
        .send()?;

    let body = (request.to_string)(response)?;

    request.cache.insert(key, body.clone());

    let des = (request.deserialize)(&body)?;

    Ok(des)
}
