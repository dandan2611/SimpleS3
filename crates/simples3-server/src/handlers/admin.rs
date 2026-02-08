use crate::AppState;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Response};
use axum::Json;
use http::StatusCode;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Serialize)]
struct BucketInfo {
    name: String,
    creation_date: String,
    anonymous_read: bool,
}

#[derive(Serialize)]
struct CredentialInfo {
    access_key_id: String,
    secret_access_key: String,
    description: String,
    created: String,
    active: bool,
}

#[derive(Deserialize)]
pub struct CreateCredentialRequest {
    pub description: Option<String>,
}

#[derive(Deserialize)]
pub struct SetAnonymousRequest {
    pub enabled: bool,
}

// --- Bucket admin endpoints ---

pub async fn admin_create_bucket(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Response<Body> {
    match state.metadata.create_bucket(&name) {
        Ok(_) => {
            if let Err(e) = state.filestore.create_bucket_dir(&name).await {
                return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
            }
            StatusCode::CREATED.into_response()
        }
        Err(e) => e.into_response(),
    }
}

pub async fn admin_list_buckets(State(state): State<Arc<AppState>>) -> Response<Body> {
    match state.metadata.list_buckets() {
        Ok(buckets) => {
            let infos: Vec<BucketInfo> = buckets
                .into_iter()
                .map(|b| BucketInfo {
                    name: b.name,
                    creation_date: b.creation_date.to_rfc3339(),
                    anonymous_read: b.anonymous_read,
                })
                .collect();
            Json(infos).into_response()
        }
        Err(e) => e.into_response(),
    }
}

pub async fn admin_delete_bucket(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Response<Body> {
    match state.metadata.delete_bucket(&name) {
        Ok(()) => {
            if let Err(e) = state.filestore.delete_bucket_dir(&name).await {
                return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
            }
            StatusCode::NO_CONTENT.into_response()
        }
        Err(e) => e.into_response(),
    }
}

pub async fn admin_set_anonymous(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Json(body): Json<SetAnonymousRequest>,
) -> Response<Body> {
    match state.metadata.set_bucket_anonymous_read(&name, body.enabled) {
        Ok(()) => StatusCode::OK.into_response(),
        Err(e) => e.into_response(),
    }
}

// --- Credential admin endpoints ---

pub async fn admin_create_credential(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateCredentialRequest>,
) -> Response<Body> {
    let access_key_id = simples3_core::auth::credentials::generate_access_key_id();
    let secret_access_key = simples3_core::auth::credentials::generate_secret_access_key();
    let description = body.description.unwrap_or_default();

    match state
        .metadata
        .create_credential(&access_key_id, &secret_access_key, &description)
    {
        Ok(record) => {
            let info = CredentialInfo {
                access_key_id: record.access_key_id,
                secret_access_key: record.secret_access_key,
                description: record.description,
                created: record.created.to_rfc3339(),
                active: record.active,
            };
            (StatusCode::CREATED, Json(info)).into_response()
        }
        Err(e) => e.into_response(),
    }
}

pub async fn admin_list_credentials(State(state): State<Arc<AppState>>) -> Response<Body> {
    match state.metadata.list_credentials() {
        Ok(creds) => {
            let infos: Vec<CredentialInfo> = creds
                .into_iter()
                .map(|c| CredentialInfo {
                    access_key_id: c.access_key_id,
                    // Don't expose secrets in list
                    secret_access_key: "********".into(),
                    description: c.description,
                    created: c.created.to_rfc3339(),
                    active: c.active,
                })
                .collect();
            Json(infos).into_response()
        }
        Err(e) => e.into_response(),
    }
}

pub async fn admin_revoke_credential(
    State(state): State<Arc<AppState>>,
    Path(access_key_id): Path<String>,
) -> Response<Body> {
    match state.metadata.revoke_credential(&access_key_id) {
        Ok(()) => StatusCode::OK.into_response(),
        Err(e) => e.into_response(),
    }
}
