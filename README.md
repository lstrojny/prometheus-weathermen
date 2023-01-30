# Prometheus Weathermen [![Build Pipeline](https://github.com/lstrojny/prometheus-weathermen/actions/workflows/build.yml/badge.svg)](https://github.com/lstrojny/prometheus-weathermen/actions/workflows/build.yml) ![crates.io](https://img.shields.io/crates/v/prometheus-weathermen.svg)

<font size=4><blockquote><em>“You don't need a weatherman to know which way the wind blows”</em><br>&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;— Bob Dylan, Subterranean Homesick Blues</blockquote></font>

A prometheus exporter endpoint for weather data or my excuse to do some Rust for real.

### What it does

Provides a Prometheus metrics endpoint on `<host>:36333/metrics` and serves the following metrics for configured
location from each configured provider:

-   `weather_temperature_celsius`
-   `weather_relative_humidity_ratio`

### Supported providers

The following services are implemented as providers. Each configured provider is queried for weather information.

| Provider                                      | Resolution | Coverage  | Supports humidity | Registration required |
| --------------------------------------------- | ---------- | --------- | ----------------- | --------------------- |
| [Meteoblue](https://www.meteoblue.com/)       | High       | Worldwide | ❌                | Yes                   |
| [OpenWeather](https://openweathermap.org/)    | Medium     | Worldwide | ✅                | Yes                   |
| [tomorrow.io](https://www.tomorrow.io/)       | High       | Worldwide | ✅                | Yes                   |
| [Deutscher Wetterdienst](https://www.dwd.de/) | Medium     | Germany   | ✅                | No                    |

You need to register an account for those providers that require an API key.

### Installation

#### Pre-built containers

Readymade containers are available for `linux/amd64`, `linux/arm64` and `linux/arm/v7`. Download
[weathermen.toml.dist](weathermen.toml.dist) to `weathermen.toml` into the current folder and adjust the configuration.

This is how to run the container using Docker:

```
docker run -p 36333:36333 \
    -v $(pwd)/weathermen.toml:/etc/prometheus-weathermen/weathermen.toml \
    lstrojny/prometheus-weathermen:latest
```

The container is also available from the GitHub container registry via `ghcr.io/lstrojny/prometheus-weathermen`.

#### Pre-built binaries

Go to [the latest release](https://github.com/lstrojny/prometheus-weathermen/releases/latest) and download the
appropriate binary for your platform.

The following platforms are supported:

| Platform     | Use case                          |
| ------------ | --------------------------------- |
| arm-linux    | 32 bit armhf, e.g. Raspberry Pi   |
| arm64-linux  | 64 bit arm, Raspberry Pi 4        |
| x86_64-linux | 64 bit X86 architecture for Linux |
| intel-mac    | Intel based Macs                  |
| arm-mac      | M1/M2 based ARM Macs              |

Please open an issue if your favorite platform is missing. It’s probably not terribly much work to get it going.

There are `dbg` (debug) variants of the binaries. These are unstripped debug builds. If you don’t know what that is, you
don’t want it.

For the Linux builds, `static` variants based on [musl libc](https://www.musl-libc.org/) are available. These are
statically linked binaries that can be used to run `prometheus-weathermen` in a container with minimal fuzz. The non-static
counterparts for Linux are build against glibc.

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

### License

This project is distributed under either:

-   The Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or https://www.apache.org/licenses/LICENSE-2.0)
-   The MIT license ([LICENSE-MIT](LICENSE-MIT) or https://opensource.org/licenses/MIT)
