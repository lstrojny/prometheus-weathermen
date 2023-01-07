# Prometheus Weathermen

> You don't need a weatherman to know which way the wind blows — Bob Dylan, Subterranean Homesick Blues

A prometheus exporter endpoint for weather data or my excuse to do some Rust for real.


### What it does

Provides a Prometheus metrics endpoint on `<host>:36333/metrics` and serves the following metrics for configured 
location from each configured provider:
 * `weather_temperature_celsius`
 * `weather_relative_humidity` (not yet implemented)

### Configuration
Check [weathermen.toml.dist](weathermen.toml.dist) for configuration options.
