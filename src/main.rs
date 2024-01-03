#![deny(clippy::all)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]
#![deny(clippy::perf)]
#![deny(clippy::style)]
#![deny(clippy::complexity)]
#![deny(clippy::correctness)]
#![warn(clippy::missing_const_for_fn)]
#![warn(clippy::cargo)]
#![warn(clippy::cargo_common_metadata)]
#![warn(clippy::unwrap_used)]
#![allow(clippy::no_effect_underscore_binding)]
#![allow(clippy::ignored_unit_patterns)]
#![allow(soft_unstable)]
#![deny(clippy::absolute_paths)]
#![deny(clippy::alloc_instead_of_core)]
#![deny(clippy::allow_attributes_without_reason)]
#![deny(clippy::arithmetic_side_effects)]
#![deny(clippy::as_conversions)]
#![deny(clippy::as_underscore)]
#![deny(clippy::assertions_on_result_states)]
#![deny(clippy::big_endian_bytes)]
#![deny(clippy::clone_on_ref_ptr)]
#![deny(clippy::create_dir)]
#![deny(clippy::dbg_macro)]
#![deny(clippy::decimal_literal_representation)]
#![deny(clippy::default_numeric_fallback)]
#![deny(clippy::default_union_representation)]
#![deny(clippy::deref_by_slicing)]
#![deny(clippy::else_if_without_else)]
#![deny(clippy::empty_drop)]
#![deny(clippy::empty_structs_with_brackets)]
#![deny(clippy::error_impl_error)]
#![deny(clippy::exhaustive_enums)]
#![deny(clippy::exhaustive_structs)]
#![deny(clippy::filetype_is_file)]
#![deny(clippy::float_cmp_const)]
#![deny(clippy::fn_to_numeric_cast_any)]
#![deny(clippy::format_push_string)]
#![deny(clippy::get_unwrap)]
#![deny(clippy::host_endian_bytes)]
#![deny(clippy::if_then_some_else_none)]
#![deny(clippy::impl_trait_in_params)]
#![deny(clippy::indexing_slicing)]
#![deny(clippy::infinite_loop)]
#![deny(clippy::inline_asm_x86_att_syntax)]
#![deny(clippy::inline_asm_x86_intel_syntax)]
#![deny(clippy::integer_division)]
#![deny(clippy::iter_over_hash_type)]
#![deny(clippy::large_include_file)]
#![deny(clippy::let_underscore_untyped)]
#![deny(clippy::little_endian_bytes)]
#![deny(clippy::lossy_float_literal)]
#![deny(clippy::map_err_ignore)]
#![deny(clippy::mem_forget)]
#![deny(clippy::missing_assert_message)]
#![deny(clippy::missing_asserts_for_indexing)]
#![deny(clippy::missing_inline_in_public_items)]
#![deny(clippy::mixed_read_write_in_expression)]
#![deny(clippy::modulo_arithmetic)]
#![deny(clippy::multiple_inherent_impl)]
#![deny(clippy::multiple_unsafe_ops_per_block)]
#![deny(clippy::mutex_atomic)]
#![deny(clippy::needless_raw_strings)]
#![deny(clippy::non_ascii_literal)]
#![deny(clippy::panic)]
#![deny(clippy::panic_in_result_fn)]
#![deny(clippy::partial_pub_fields)]
#![deny(clippy::print_stderr)]
#![deny(clippy::print_stdout)]
#![deny(clippy::pub_without_shorthand)]
#![deny(clippy::rc_buffer)]
#![deny(clippy::rc_mutex)]
#![deny(clippy::ref_patterns)]
#![deny(clippy::rest_pat_in_fully_bound_structs)]
#![deny(clippy::same_name_method)]
#![deny(clippy::self_named_module_files)]
#![deny(clippy::semicolon_inside_block)]
#![deny(clippy::semicolon_outside_block)]
#![deny(clippy::shadow_reuse)]
#![deny(clippy::shadow_unrelated)]
#![deny(clippy::single_char_lifetime_names)]
#![deny(clippy::str_to_string)]
#![deny(clippy::string_add)]
#![deny(clippy::string_lit_chars_any)]
#![deny(clippy::string_slice)]
#![deny(clippy::string_to_string)]
#![deny(clippy::suspicious_xor_used_as_pow)]
#![deny(clippy::tests_outside_test_module)]
#![deny(clippy::todo)]
#![deny(clippy::try_err)]
#![deny(clippy::undocumented_unsafe_blocks)]
#![deny(clippy::unimplemented)]
#![deny(clippy::unnecessary_safety_comment)]
#![deny(clippy::unnecessary_safety_doc)]
#![deny(clippy::unnecessary_self_imports)]
#![deny(clippy::unneeded_field_pattern)]
#![deny(clippy::unreachable)]
#![deny(clippy::unseparated_literal_suffix)]
#![deny(clippy::unwrap_in_result)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::use_debug)]
#![deny(clippy::verbose_file_reads)]
#![deny(clippy::wildcard_enum_match_arm)]
#![cfg_attr(feature = "nightly", feature(test))]

use crate::config::{read, DEFAULT_CONFIG};
use crate::error::exit_if_handle_fatal;
use clap::{arg, command, Parser};
#[cfg(debug_assertions)]
use log::Level;
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
pub struct DebugLevel;

#[cfg(debug_assertions)]
impl clap_verbosity_flag::LogLevel for DebugLevel {
    fn default() -> Option<Level> {
        Some(Level::Debug)
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
