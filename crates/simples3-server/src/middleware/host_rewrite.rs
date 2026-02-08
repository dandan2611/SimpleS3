use crate::AppState;
use axum::{
    body::Body,
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use std::sync::Arc;

/// Rewrites virtual-host style requests to path-style.
/// e.g. `Host: mybucket.s3.localhost` + `GET /mykey` → `GET /mybucket/mykey`
pub async fn host_rewrite_middleware(
    State(state): State<Arc<AppState>>,
    mut request: Request<Body>,
    next: Next,
) -> Response {
    let hostname = &state.config.hostname;

    if let Some(host) = request.headers().get("host").and_then(|v| v.to_str().ok()) {
        // Strip port if present
        let host_no_port = host.split(':').next().unwrap_or(host);

        // Check if host is `bucket.hostname`
        if let Some(bucket) = host_no_port.strip_suffix(&format!(".{}", hostname)) {
            if !bucket.is_empty() {
                let old_path = request.uri().path().to_string();
                let query = request.uri().query().map(|q| format!("?{}", q)).unwrap_or_default();
                let new_path = format!("/{}{}{}", bucket, old_path, query);

                let new_uri: http::Uri = new_path.parse().unwrap_or_else(|_| request.uri().clone());
                *request.uri_mut() = new_uri;
            }
        }
    }

    next.run(request).await
}
