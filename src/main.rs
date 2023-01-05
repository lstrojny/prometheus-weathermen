extern crate core;

use crate::config::parse;
use crate::prometheus::prometheus_metrics;
use crate::provider::WeatherRequest;
use rocket::tokio::task;
use rocket::tokio::task::JoinSet;
use rocket::{get, launch, routes};

mod config;
mod prometheus;
mod provider;

#[get("/")]
async fn index() -> String {
    let config = match parse() {
        Ok(config) => config,
        Err(err) => panic!("{}", err),
    };

    println!("Config dump {config:?}");

    let provider = match config.provider {
        Some(provider) => provider,
        None => return "# No provider defined".to_owned(),
    };

    let mut set = JoinSet::new();
    for p in provider {
        let locations = config.location.to_owned();
        for (name, location) in locations {
            let task_provider = p.clone();
            set.spawn(task::spawn_blocking(move || {
                task_provider.for_coordinates(WeatherRequest {
                    name: location.name.unwrap_or(name),
                    coordinates: location.coordinates,
                })
            }));
        }
    }

    let mut metrics = vec![];

    while let Some(result) = set.join_next().await {
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
    rocket::build().mount("/", routes![index])
}
