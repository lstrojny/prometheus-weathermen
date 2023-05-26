#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::missing_const_for_fn)]
#![warn(clippy::cargo)]
#![warn(clippy::cargo_common_metadata)]
#![warn(clippy::unwrap_used)]
#![allow(clippy::let_underscore_untyped)]
#![allow(clippy::no_effect_underscore_binding)]

use crate::config::{read, DEFAULT_CONFIG};
use crate::error::exit_if_handle_fatal;
use clap::{arg, command, Parser};
use rocket::{launch, Build, Rocket};
use std::path::PathBuf;

mod config;
mod error;
mod http_server;
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

#[launch]
pub async fn start_server() -> Rocket<Build> {
    let args = Args::parse();

    let log_level = args
        .verbose
        .log_level()
        .expect("Log level cannot be not available");

    logging::init(log_level).expect("Logging successfully initialized");

    let config = read(args.config, log_level).unwrap_or_else(exit_if_handle_fatal);

    http_server::configure_rocket(config).await
}
