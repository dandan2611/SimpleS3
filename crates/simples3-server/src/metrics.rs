use std::sync::OnceLock;

use metrics_exporter_prometheus::PrometheusHandle;

pub const REQUEST_COUNTER: &str = "s3_requests_total";
pub const REQUEST_DURATION: &str = "s3_request_duration_seconds";
pub const ERROR_COUNTER: &str = "s3_errors_total";
pub const MULTIPART_EXPIRED_TOTAL: &str = "simples3_multipart_expired_total";
pub const MULTIPART_ACTIVE_UPLOADS: &str = "simples3_active_multipart_uploads";
pub const MULTIPART_TOTAL_PARTS: &str = "simples3_multipart_total_parts";
pub const MULTIPART_OLDEST_AGE_SECONDS: &str = "simples3_multipart_oldest_age_seconds";

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
