#![feature(absolute_path)]
extern crate core;

use crate::config::{get_provider_tasks, ProviderTasks};
use crate::prometheus::prometheus_metrics;
use crate::providers::Weather;
use clap::Parser;
use clap_verbosity_flag::WarnLevel;
use log::{error, info};
use rocket::tokio::task;
use rocket::tokio::task::JoinSet;
use rocket::{get, launch, routes, State};
use std::process::exit;
use tokio::task::JoinError;

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
            info!(
                "Requesting weather data for {:?} from {:?} ({:?}",
                prov_req.name,
                provider.id(),
                prov_req.query,
            );
            provider.for_coordinates(&task_cache, &prov_req)
        }));
    }

    wait_for_metrics(join_set).await.unwrap_or_else(|e| {
        error!("Error while requesting weather data {e}");
        "".to_string()
    })
}

async fn wait_for_metrics(
    mut join_set: JoinSet<Result<anyhow::Result<Weather>, JoinError>>,
) -> anyhow::Result<String> {
    let mut metrics = vec![];

    while let Some(result) = join_set.join_next().await {
        metrics.push(prometheus_metrics(result???)?);
    }

    Ok(metrics.join("\n"))
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[clap(flatten)]
    verbose: clap_verbosity_flag::Verbosity<WarnLevel>,
}

#[launch]
fn rocket() -> _ {
    let args = Args::parse();

    stderrlog::new()
        .module(module_path!())
        .verbosity(args.verbose.log_level().unwrap())
        .timestamp(stderrlog::Timestamp::Millisecond)
        .init()
        .unwrap();

    let tasks = get_provider_tasks().unwrap_or_else(|e| {
        error!("Fatal error: {e}");
        exit(1);
    });

    rocket::build().manage(tasks).mount("/", routes![index])
}
