use std::sync::OnceLock;

use metrics_exporter_prometheus::PrometheusHandle;

pub const REQUEST_COUNTER: &str = "s3_requests_total";
pub const REQUEST_DURATION: &str = "s3_request_duration_seconds";
pub const ERROR_COUNTER: &str = "s3_errors_total";

static HANDLE: OnceLock<PrometheusHandle> = OnceLock::new();

pub fn init_metrics() -> PrometheusHandle {
    HANDLE
        .get_or_init(|| {
            metrics_exporter_prometheus::PrometheusBuilder::new()
                .install_recorder()
                .expect("Failed to install Prometheus recorder")
        })
        .clone()
}
