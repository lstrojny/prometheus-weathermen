#![feature(absolute_path)]
extern crate core;

use crate::config::{parse, Config};
use crate::prometheus::prometheus_metrics;
use crate::providers::WeatherRequest;
use rocket::tokio::task;
use rocket::tokio::task::JoinSet;
use rocket::{get, launch, routes, State};

mod config;
mod prometheus;
mod providers;

#[get("/")]
async fn index(config: &State<Config>) -> String {
    let configured_providers = match config.providers.clone() {
        Some(providers) => providers,
        None => return "# No providers defined".to_owned(),
    };

    let mut tasks = JoinSet::new();

    for configured_provider in configured_providers {
        let locations = config.locations.clone();
        for (name, location) in locations {
            let configured_provider_for_task = configured_provider.clone();
            tasks.spawn(task::spawn_blocking(move || {
                configured_provider_for_task.for_coordinates(WeatherRequest {
                    name: location.name.unwrap_or(name),
                    coordinates: location.coordinates,
                })
            }));
        }
    }

    let mut metrics = vec![];

    while let Some(result) = tasks.join_next().await {
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

#[launch]
fn rocket() -> _ {
    let config = match parse() {
        Ok(config) => config,
        Err(err) => panic!("{}", err),
    };
    println!("Config dump {config:?}");

    rocket::build().manage(config).mount("/", routes![index])
}
