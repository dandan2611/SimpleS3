use crate::handlers;
use crate::middleware::admin_auth::admin_auth_middleware;
use crate::middleware::auth::auth_middleware;
use crate::middleware::host_rewrite::host_rewrite_middleware;
use crate::AppState;
use axum::{
    Router,
    body::Body,
    extract::{Request, State},
    middleware as axum_mw,
    response::Response,
    routing::{delete, get, post, put},
};
use simples3_core::s3::request::{parse_s3_operation, S3Operation};
use std::collections::HashMap;
use std::sync::Arc;

async fn s3_dispatcher(
    State(state): State<Arc<AppState>>,
    request: Request<Body>,
) -> Response<Body> {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let path = uri.path().to_string();

    // Parse query params
    let query: HashMap<String, String> = uri
        .query()
        .map(|q| {
            url_query_pairs(q)
        })
        .unwrap_or_default();

    let operation = match parse_s3_operation(&method, &path, &query) {
        Some(op) => op,
        None => {
            return simples3_core::S3Error::InvalidArgument("Unknown operation".into())
                .into_response();
        }
    };

    tracing::debug!(?operation, "Dispatching S3 operation");

    match operation {
        S3Operation::ListBuckets => handlers::bucket::list_buckets(state).await,
        S3Operation::CreateBucket { bucket } => {
            handlers::bucket::create_bucket(state, &bucket).await
        }
        S3Operation::DeleteBucket { bucket } => {
            handlers::bucket::delete_bucket(state, &bucket).await
        }
        S3Operation::HeadBucket { bucket } => {
            handlers::bucket::head_bucket(state, &bucket).await
        }
        S3Operation::ListObjectsV2 { bucket } => {
            handlers::object::list_objects_v2(state, &bucket, &query).await
        }
        S3Operation::PutObject { bucket, key } => {
            if request.headers().contains_key("x-amz-copy-source") {
                handlers::object::copy_object(state, &bucket, &key, request).await
            } else {
                handlers::object::put_object(state, &bucket, &key, request).await
            }
        }
        S3Operation::GetObject { bucket, key } => {
            handlers::object::get_object(state, &bucket, &key).await
        }
        S3Operation::HeadObject { bucket, key } => {
            handlers::object::head_object(state, &bucket, &key).await
        }
        S3Operation::DeleteObject { bucket, key } => {
            handlers::object::delete_object(state, &bucket, &key).await
        }
        S3Operation::CreateMultipartUpload { bucket, key } => {
            handlers::multipart::create_multipart_upload(state, &bucket, &key).await
        }
        S3Operation::UploadPart {
            bucket,
            key,
            upload_id,
            part_number,
        } => {
            handlers::multipart::upload_part(state, &bucket, &key, &upload_id, part_number, request)
                .await
        }
        S3Operation::CompleteMultipartUpload {
            bucket,
            key,
            upload_id,
        } => {
            handlers::multipart::complete_multipart_upload(state, &bucket, &key, &upload_id, request)
                .await
        }
        S3Operation::AbortMultipartUpload {
            bucket: _,
            key: _,
            upload_id,
        } => handlers::multipart::abort_multipart_upload(state, &upload_id).await,
        S3Operation::ListParts {
            bucket: _,
            key: _,
            upload_id,
        } => handlers::multipart::list_parts(state, &upload_id).await,
        S3Operation::PutObjectTagging { bucket, key } => {
            handlers::object::put_object_tagging(state, &bucket, &key, request).await
        }
        S3Operation::GetObjectTagging { bucket, key } => {
            handlers::object::get_object_tagging(state, &bucket, &key).await
        }
        S3Operation::DeleteObjectTagging { bucket, key } => {
            handlers::object::delete_object_tagging(state, &bucket, &key).await
        }
        S3Operation::DeleteObjects { bucket } => {
            handlers::object::delete_objects(state, &bucket, request).await
        }
    }
}

fn url_query_pairs(query: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for pair in query.split('&') {
        if pair.is_empty() {
            continue;
        }
        let mut parts = pair.splitn(2, '=');
        let key = parts.next().unwrap_or("");
        let value = parts.next().unwrap_or("");
        let key = percent_decode(key);
        let value = percent_decode(value);
        map.insert(key, value);
    }
    map
}

fn percent_decode(s: &str) -> String {
    percent_encoding::percent_decode_str(s)
        .decode_utf8_lossy()
        .into_owned()
}

use axum::response::IntoResponse;

pub fn build_s3_router(state: Arc<AppState>) -> Router {
    Router::new()
        .fallback(s3_dispatcher)
        .layer(axum_mw::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .layer(axum_mw::from_fn_with_state(
            state.clone(),
            host_rewrite_middleware,
        ))
        .with_state(state)
}

pub fn build_admin_router(state: Arc<AppState>) -> Router {
    let admin_routes = Router::new()
        .route("/buckets", get(handlers::admin::admin_list_buckets))
        .route(
            "/buckets/{name}",
            put(handlers::admin::admin_create_bucket)
                .delete(handlers::admin::admin_delete_bucket),
        )
        .route(
            "/buckets/{name}/anonymous",
            put(handlers::admin::admin_set_anonymous),
        )
        .route(
            "/credentials",
            get(handlers::admin::admin_list_credentials)
                .post(handlers::admin::admin_create_credential),
        )
        .route(
            "/credentials/{access_key_id}",
            delete(handlers::admin::admin_revoke_credential),
        )
        .layer(axum_mw::from_fn_with_state(
            state.clone(),
            admin_auth_middleware,
        ))
        .with_state(state);

    Router::new().nest("/_admin", admin_routes)
}
