#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::missing_const_for_fn)]
#![warn(clippy::cargo)]
#![warn(clippy::cargo_common_metadata)]
#![allow(clippy::no_effect_underscore_binding)]
#![feature(absolute_path)]
extern crate core;

use crate::config::{get_provider_tasks, read, DEFAULT_CONFIG};
use crate::http::{index, metrics};
use clap::{arg, command, Parser};
use log::{debug, error};
use rocket::{launch, routes};
use std::path::PathBuf;
use std::process::exit;

mod config;
mod http;
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

#[launch]
fn rocket() -> _ {
    let args = Args::parse();

    let log_level = args.verbose.log_level().unwrap();

    stderrlog::new()
        .verbosity(log_level)
        .timestamp(stderrlog::Timestamp::Millisecond)
        .init()
        .unwrap();

    debug!("Configured logger with level {log_level:?}");

    let config = read(args.config, log_level).unwrap_or_else(|e| {
        error!("Fatal error: {e}");
        exit(1);
    });

    let tasks = get_provider_tasks(config.clone()).unwrap_or_else(|e| {
        error!("Fatal error: {e}");
        exit(1);
    });

    rocket::custom(config.http)
        .manage(tasks)
        .manage(config.auth)
        .mount("/", routes![index, metrics])
}
