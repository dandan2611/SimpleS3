use crate::AppState;
use axum::body::Body;
use axum::extract::Request;
use axum::response::{IntoResponse, Response};
use http::StatusCode;
use simples3_core::s3::types::BucketPolicy;
use std::sync::Arc;

pub async fn put_bucket_policy(
    state: Arc<AppState>,
    bucket: &str,
    request: Request<Body>,
) -> Response<Body> {
    let body_bytes = match axum::body::to_bytes(request.into_body(), state.config.max_policy_body_size).await {
        Ok(b) => b,
        Err(e) => return simples3_core::S3Error::InternalError(e.to_string()).into_response(),
    };

    let policy: BucketPolicy = match serde_json::from_slice(&body_bytes) {
        Ok(p) => p,
        Err(e) => {
            return simples3_core::S3Error::InvalidArgument(format!(
                "Invalid policy JSON: {}",
                e
            ))
            .into_response();
        }
    };

    if policy.statements.is_empty() {
        return simples3_core::S3Error::InvalidArgument(
            "Policy must contain at least one statement".to_string(),
        )
        .into_response();
    }

    match state.metadata.put_bucket_policy(bucket, &policy) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => e.into_response(),
    }
}

pub async fn get_bucket_policy(
    state: Arc<AppState>,
    bucket: &str,
) -> Response<Body> {
    match state.metadata.get_bucket_policy(bucket) {
        Ok(policy) => {
            let body = serde_json::to_string(&policy).unwrap();
            (
                StatusCode::OK,
                [("content-type", "application/json")],
                body,
            )
                .into_response()
        }
        Err(e) => e.into_response(),
    }
}

pub async fn delete_bucket_policy(
    state: Arc<AppState>,
    bucket: &str,
) -> Response<Body> {
    match state.metadata.delete_bucket_policy(bucket) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => e.into_response(),
    }
}
