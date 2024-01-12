#![cfg_attr(feature = "nightly", feature(test))]

use crate::config::{read, DEFAULT_CONFIG};
use crate::error::exit_if_handle_fatal;
use clap::{arg, command, Parser};
use rocket::{launch, Build, Rocket};
use std::path::PathBuf;

mod authentication;
mod config;
mod error;
mod http_server;
mod logging;
mod prometheus;
mod providers;

#[cfg(debug_assertions)]
#[derive(Copy, Clone, Debug, Default)]
struct DebugLevel;

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

/// Start the HTTP server to serve Prometheus metrics
///
/// # Panics
///
/// Will panic if the log level cannot be parsed
#[launch]
async fn start_server() -> Rocket<Build> {
    let args = Args::parse();

    let log_level = args
        .verbose
        .log_level()
        .expect("Log level cannot be not available");

    logging::init(log_level).expect("Logging successfully initialized");

    let config = read(args.config, log_level).unwrap_or_else(exit_if_handle_fatal);

    http_server::configure_rocket(config).await
}
