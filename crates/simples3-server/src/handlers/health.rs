use crate::AppState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use std::sync::Arc;

pub async fn health() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

pub async fn ready(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    // Check sled is accessible
    if let Err(e) = state.metadata.list_buckets() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            format!("metadata store unavailable: {}", e),
        );
    }

    // Check filesystem is accessible by writing and removing a probe file
    let probe_path = state.config.data_dir.join(".ready-probe");
    if let Err(e) = std::fs::write(&probe_path, b"probe") {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            format!("data directory not writable: {}", e),
        );
    }
    let _ = std::fs::remove_file(&probe_path);

    (StatusCode::OK, "ready".to_string())
}

pub async fn metrics_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    // Collect storage gauges on-demand
    if let Ok(buckets) = state.metadata.list_buckets() {
        metrics::gauge!("simples3_bucket_count").set(buckets.len() as f64);

        let mut total_objects: u64 = 0;
        let mut total_bytes: u64 = 0;
        for bucket in &buckets {
            if let Ok(resp) = state.metadata.list_objects_v2(
                &simples3_core::s3::types::ListObjectsV2Request {
                    bucket: bucket.name.clone(),
                    prefix: String::new(),
                    delimiter: String::new(),
                    max_keys: u32::MAX,
                    continuation_token: None,
                    start_after: None,
                },
            ) {
                total_objects += resp.contents.len() as u64;
                total_bytes += resp.contents.iter().map(|o| o.size).sum::<u64>();
            }
        }
        metrics::gauge!("simples3_total_object_count").set(total_objects as f64);
        metrics::gauge!("simples3_total_storage_bytes").set(total_bytes as f64);
    }

    if let Ok(creds) = state.metadata.list_credentials() {
        metrics::gauge!("simples3_credential_count").set(creds.len() as f64);
    }

    if let Ok(uploads) = state.metadata.list_multipart_uploads() {
        metrics::gauge!(crate::metrics::MULTIPART_ACTIVE_UPLOADS).set(uploads.len() as f64);
        let total_parts: usize = uploads.iter().map(|u| u.parts.len()).sum();
        metrics::gauge!(crate::metrics::MULTIPART_TOTAL_PARTS).set(total_parts as f64);
        let oldest_age = uploads
            .iter()
            .map(|u| chrono::Utc::now().signed_duration_since(u.created).num_seconds().max(0) as f64)
            .reduce(f64::max)
            .unwrap_or(0.0);
        metrics::gauge!(crate::metrics::MULTIPART_OLDEST_AGE_SECONDS).set(oldest_age);
    }

    let uptime = state.start_time.elapsed().as_secs_f64();
    metrics::gauge!("simples3_uptime_seconds").set(uptime);

    let output = state.metrics_handle.render();
    (
        StatusCode::OK,
        [("content-type", "text/plain; version=0.0.4; charset=utf-8")],
        output,
    )
}
