use crate::AppState;
use axum::body::Body;
use axum::response::{IntoResponse, Response};
use http::StatusCode;
use simples3_core::s3::xml;
use std::sync::Arc;

pub async fn create_bucket(state: Arc<AppState>, bucket: &str) -> Response<Body> {
    match state.metadata.create_bucket(bucket) {
        Ok(_) => {
            if let Err(e) = state.filestore.create_bucket_dir(bucket).await {
                return e.into_response();
            }
            (
                StatusCode::OK,
                [("location", format!("/{}", bucket).as_str())],
                "",
            )
                .into_response()
        }
        Err(e) => e.into_response(),
    }
}

pub async fn list_buckets(state: Arc<AppState>) -> Response<Body> {
    match state.metadata.list_buckets() {
        Ok(buckets) => {
            let body = xml::list_buckets_xml("simples3", &buckets);
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

pub async fn delete_bucket(state: Arc<AppState>, bucket: &str) -> Response<Body> {
    match state.metadata.delete_bucket(bucket) {
        Ok(()) => {
            if let Err(e) = state.filestore.delete_bucket_dir(bucket).await {
                return e.into_response();
            }
            StatusCode::NO_CONTENT.into_response()
        }
        Err(e) => e.into_response(),
    }
}

pub async fn head_bucket(state: Arc<AppState>, bucket: &str) -> Response<Body> {
    match state.metadata.get_bucket(bucket) {
        Ok(_) => StatusCode::OK.into_response(),
        Err(e) => e.into_response(),
    }
}
