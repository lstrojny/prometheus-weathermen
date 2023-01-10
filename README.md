# Prometheus Weathermen [![Build Pipeline](https://github.com/lstrojny/prometheus-weathermen/actions/workflows/build.yml/badge.svg)](https://github.com/lstrojny/prometheus-weathermen/actions/workflows/build.yml)

<font size=4><blockquote><em>“You don't need a weatherman to know which way the wind blows”</em><br>&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;— Bob Dylan, Subterranean Homesick Blues</blockquote></font>

A prometheus exporter endpoint for weather data or my excuse to do some Rust for real.

### What it does

Provides a Prometheus metrics endpoint on `<host>:36333/metrics` and serves the following metrics for configured
location from each configured provider:

-   `weather_temperature_celsius`
-   `weather_relative_humidity_ratio`

### Supported providers

The following services are implemented as providers. Each configured provider is queries for weather information.

-   [Meteoblue](https://www.meteoblue.com/)
-   [OpenWeather](https://openweathermap.org/)
-   [tomorrow.io](https://www.tomorrow.io/)

### Configuration

Check [weathermen.toml.dist](weathermen.toml.dist) for configuration options.

Configuration values can also be set from environment variables with the prefix `PROMW_`. For example, to set the HTTP
port from an environment variable, use `PROMW_HTTP__PORT=12345`. The double underscore is not a typo, it is necessary
to disambiguate hierarchy from name. Assume this TOML config:

```toml
[provider.open_weather]
api_key = "XYZ"
```

The corresponding env variable would be `PROMW_PROVIDER__OPEN_WEATHER__API_KEY`.
