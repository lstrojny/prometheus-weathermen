use log::error;
use std::fmt::Display;
use std::process::exit;

pub fn exit_if_handle_fatal<E, R>(error: E) -> R
where
    E: Display,
{
    error!("Fatal error: {error}");

    exit(1)
}
