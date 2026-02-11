use crate::AppState;
use axum::body::Body;
use axum::extract::Request;
use axum::response::{IntoResponse, Response};
use http::StatusCode;
use simples3_core::s3::xml;
use std::sync::Arc;

pub async fn put_lifecycle_configuration(
    state: Arc<AppState>,
    bucket: &str,
    request: Request<Body>,
) -> Response<Body> {
    let body_bytes = match axum::body::to_bytes(request.into_body(), state.config.max_xml_body_size).await {
        Ok(b) => b,
        Err(e) => return simples3_core::S3Error::InternalError(e.to_string()).into_response(),
    };

    let config = match xml::parse_lifecycle_configuration_xml(&body_bytes) {
        Ok(c) => c,
        Err(e) => return e.into_response(),
    };

    match state.metadata.put_lifecycle_configuration(bucket, &config) {
        Ok(()) => StatusCode::OK.into_response(),
        Err(e) => e.into_response(),
    }
}

pub async fn get_lifecycle_configuration(
    state: Arc<AppState>,
    bucket: &str,
) -> Response<Body> {
    match state.metadata.get_lifecycle_configuration(bucket) {
        Ok(config) => {
            let body = xml::lifecycle_configuration_xml(&config);
            (
                StatusCode::OK,
                [("content-type", "application/xml")],
                body,
            )
                .into_response()
        }
        Err(e) => e.into_response(),
    }
}

pub async fn delete_lifecycle_configuration(
    state: Arc<AppState>,
    bucket: &str,
) -> Response<Body> {
    match state.metadata.delete_lifecycle_configuration(bucket) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => e.into_response(),
    }
}
