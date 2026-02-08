use axum::body::Body;
use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;
use std::collections::HashMap;
use std::time::Instant;

use simples3_core::s3::request::parse_s3_operation;

use crate::router::url_query_pairs;

pub async fn metrics_middleware(request: Request<Body>, next: Next) -> Response {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let path = uri.path().to_string();

    let query: HashMap<String, String> = uri
        .query()
        .map(|q| url_query_pairs(q))
        .unwrap_or_default();

    let operation_name = parse_s3_operation(&method, &path, &query)
        .map(|op| op.name())
        .unwrap_or("Unknown");

    let start = Instant::now();
    let response = next.run(request).await;
    let duration = start.elapsed().as_secs_f64();

    metrics::counter!(crate::metrics::REQUEST_COUNTER, "operation" => operation_name).increment(1);
    metrics::histogram!(crate::metrics::REQUEST_DURATION, "operation" => operation_name)
        .record(duration);

    let status = response.status().as_u16();
    if status >= 400 {
        metrics::counter!(crate::metrics::ERROR_COUNTER, "status" => status.to_string())
            .increment(1);
    }

    response
}
