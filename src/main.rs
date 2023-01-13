#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::missing_const_for_fn)]
#![warn(clippy::cargo)]
#![warn(clippy::cargo_common_metadata)]
#![allow(clippy::no_effect_underscore_binding)]

use crate::config::{get_provider_tasks, read, DEFAULT_CONFIG};
use crate::http::{index, metrics};
use clap::{arg, command, Parser};
use log::error;
use rocket::{launch, routes};
use std::path::PathBuf;
use std::process::exit;
use tokio::task;

mod config;
mod http;
mod logging;
mod prometheus;
mod providers;

#[cfg(debug_assertions)]
#[derive(Copy, Clone, Debug, Default)]
pub struct DebugLevel;

#[cfg(debug_assertions)]
impl clap_verbosity_flag::LogLevel for DebugLevel {
    fn default() -> Option<log::Level> {
        Some(log::Level::Debug)
    }
}

#[cfg(debug_assertions)]
type DefaultLogLevel = DebugLevel;

#[cfg(not(debug_assertions))]
type DefaultLogLevel = clap_verbosity_flag::WarnLevel;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[clap(flatten)]
    verbose: clap_verbosity_flag::Verbosity<DefaultLogLevel>,

    // Custom config file location
    #[arg(short, long, default_value = DEFAULT_CONFIG)]
    config: PathBuf,
}

fn handle_fatal_error<E, R>(error: E) -> R
where
    E: std::fmt::Display,
{
    error!("Fatal error: {error}");

    exit(1)
}

#[launch]
async fn rocket() -> _ {
    let args = Args::parse();

    let log_level = args
        .verbose
        .log_level()
        .expect("Log level cannot be not available");

    logging::init(log_level).expect("Logging successfully initialied");

    let config = read(args.config, log_level).unwrap_or_else(handle_fatal_error);

    let config_clone = config.clone();
    let tasks = task::spawn_blocking(move || get_provider_tasks(config_clone))
        .await
        .unwrap_or_else(handle_fatal_error)
        .unwrap_or_else(handle_fatal_error);

    rocket::custom(config.http)
        .manage(tasks)
        .manage(config.auth)
        .mount("/", routes![index, metrics])
}
