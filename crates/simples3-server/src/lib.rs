pub mod handlers;
pub mod metrics;
pub mod middleware;
pub mod router;

pub struct AppState {
    pub config: simples3_core::Config,
    pub metadata: simples3_core::storage::MetadataStore,
    pub filestore: simples3_core::storage::FileStore,
    pub start_time: std::time::Instant,
    pub metrics_handle: metrics_exporter_prometheus::PrometheusHandle,
}
