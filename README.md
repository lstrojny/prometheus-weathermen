# Prometheus Weathermen [![Build Pipeline](https://github.com/lstrojny/prometheus-weathermen/actions/workflows/build.yml/badge.svg)](https://github.com/lstrojny/prometheus-weathermen/actions/workflows/build.yml)

<font size=4><blockquote><em>“You don't need a weatherman to know which way the wind blows”</em><br>&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;— Bob Dylan, Subterranean Homesick Blues</blockquote></font>

A prometheus exporter endpoint for weather data or my excuse to do some Rust for real.

### What it does

Provides a Prometheus metrics endpoint on `<host>:36333/metrics` and serves the following metrics for configured
location from each configured provider:

-   `weather_temperature_celsius`
-   `weather_relative_humidity` (not yet implemented)

### Configuration

Check [weathermen.toml.dist](weathermen.toml.dist) for configuration options.
