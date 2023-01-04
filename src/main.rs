use opentelemetry::metrics::Meter;
use opentelemetry::sdk::export::metrics::aggregation;
use opentelemetry::sdk::metrics::{controllers, processors, selectors};
use opentelemetry::sdk::Resource;
use opentelemetry::trace::FutureExt;
use opentelemetry::{global, Context, KeyValue};
use opentelemetry_prometheus::PrometheusExporter;
use prometheus::{Encoder, TextEncoder};
use rocket::{get, launch, routes};

#[get("/")]
async fn index() -> String {
    let controller = controllers::basic(processors::factory(
        selectors::simple::inexpensive(),
        aggregation::stateless_temporality_selector(),
    ))
    .with_resource(Resource::new(vec![KeyValue::new(
        "service.name",
        "prometheus-weather-exporter",
    )]))
    .build();
    let exporter = opentelemetry_prometheus::exporter(controller).init();
    let cx = Context::current();
    let meter = global::meter("foo.com/prometheus-weather-exporter");

    let temperature = meter
        .f64_up_down_counter("weather_temperature_celsius")
        .with_description("Temperature in celsius")
        .init();

    temperature.add(
        &cx,
        22.9_f64,
        &[
            KeyValue::new("latitude", format!("{:.7}", 48.137154_f64)),
            KeyValue::new("longitude", format!("{:.7}", 11.576124_f64)),
        ],
    );

    let encoder = TextEncoder::new();
    let metric_families = exporter.registry().gather();
    let mut result = Vec::new();
    encoder.encode(&metric_families, &mut result).unwrap();

    return String::from_utf8(result).unwrap();
}

#[launch]
fn rocket() -> _ {
    rocket::build().mount("/", routes![index])
}
