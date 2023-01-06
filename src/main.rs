#![feature(absolute_path)]
extern crate core;

use crate::config::{get_provider_tasks, ProviderTasks};
use crate::prometheus::prometheus_metrics;
use rocket::tokio::task;
use rocket::tokio::task::JoinSet;
use rocket::{get, launch, routes, State};

mod config;
mod prometheus;
mod providers;

#[get("/")]
async fn index(unscheduled_tasks: &State<ProviderTasks>) -> String {
    let mut join_set = JoinSet::new();

    #[allow(clippy::unnecessary_to_owned)]
    for (provider, req, cache) in unscheduled_tasks.to_vec() {
        let prov_req = req.clone();
        let task_cache = cache.clone();
        join_set.spawn(task::spawn_blocking(move || {
            provider.for_coordinates(&task_cache, &prov_req)
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

#[launch]
fn rocket() -> _ {
    let tasks = get_provider_tasks().unwrap_or_else(|e| panic!("Fatal error: {e}"));

    rocket::build().manage(tasks).mount("/", routes![index])
}
