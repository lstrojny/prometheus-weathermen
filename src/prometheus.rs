use crate::provider::provider::Weather;
use opentelemetry::sdk::export::metrics::aggregation;
use opentelemetry::sdk::metrics::{controllers, processors, selectors};
use opentelemetry::sdk::Resource;
use opentelemetry::{global, Context, KeyValue};
use prometheus::TextEncoder;

pub fn prometheus_metrics(weather: Weather) -> String {
    println!("prometheus_metrics() {:?}", weather);

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
    let meter = global::meter("foo.com/prometheus-weather-exporter");

    let temperature = meter
        .f64_up_down_counter("weather_temperature_celsius")
        .with_description("Temperature in celsius")
        .init();

    let cx = Context::current();
    temperature.add(
        &cx,
        weather.temperature.to_f32() as f64,
        &[
            KeyValue::new("city", weather.city),
            KeyValue::new("latitude", weather.coordinates.get_latitude().to_string()),
            KeyValue::new("longitude", weather.coordinates.get_longitude().to_string()),
        ],
    );

    let encoder = TextEncoder::new();
    let metric_families = exporter.registry().gather();

    encoder.encode_to_string(&metric_families).unwrap()
}
