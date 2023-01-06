#![feature(absolute_path)]
extern crate core;

use crate::config::{parse, Config};
use crate::prometheus::prometheus_metrics;
use crate::providers::{Coordinates, WeatherProvider, WeatherRequest};
use rocket::tokio::task;
use rocket::tokio::task::JoinSet;
use rocket::{get, launch, routes, State};
use std::ops::Deref;
use std::sync::Arc;

mod config;
mod prometheus;
mod providers;

#[get("/")]
async fn index(unscheduled_tasks: &State<UnscheduledTasks>, config: &State<Config>) -> String {
    let mut join_set = JoinSet::new();

    for (provider, req) in unscheduled_tasks.to_vec() {
        let prov_req = req.clone();
        join_set.spawn(task::spawn_blocking(move || {
            provider.for_coordinates(&prov_req)
        }));
    }

    let mut metrics = vec![];

    while let Some(result) = join_set.join_next().await {
        match result {
            Ok(result) => match result {
                Ok(result) => match result {
                    Ok(result) => metrics.push(prometheus_metrics(result)),
                    Err(e) => println!("Error {e:?}"),
                },
                Err(e) => println!("Error {e:?}"),
            },
            Err(e) => println!("Error {e:?}"),
        }
    }

    metrics.join("\n")
}

type UnscheduledTasks = Vec<(
    Arc<dyn WeatherProvider + Send + Sync>,
    WeatherRequest<Coordinates>,
)>;

#[launch]
fn rocket() -> _ {
    let config = match parse() {
        Ok(config) => config,
        Err(err) => panic!("{}", err),
    };
    println!("Config dump {config:?}");

    let configured_providers = match config.providers.clone() {
        Some(providers) => providers,
        None => panic!("No providers defined"),
    };

    let mut tasks: UnscheduledTasks = vec![];

    for configured_provider in configured_providers.into_iter() {
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
            ));
        }
    }

    rocket::build()
        .manage(config)
        .manage(tasks)
        .mount("/", routes![index])
}
