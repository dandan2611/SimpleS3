use crate::AppState;
use axum::{
    body::Body,
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use axum::response::IntoResponse;
use chrono::{NaiveDateTime, Utc};
use simples3_core::auth::sigv4;
use simples3_core::s3::request::{parse_s3_operation, S3Operation};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

#[derive(Clone)]
pub struct AnonymousPublicListOnly;

pub async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let path = uri.path().to_string();

    let query: HashMap<String, String> = uri
        .query()
        .map(|q| {
            q.split('&')
                .filter(|p| !p.is_empty())
                .filter_map(|p| {
                    let mut kv = p.splitn(2, '=');
                    Some((kv.next()?.to_string(), kv.next().unwrap_or("").to_string()))
                })
                .collect()
        })
        .unwrap_or_default();

    let operation = parse_s3_operation(&method, &path, &query);

    // Check for presigned URL (query-string auth)
    if query.contains_key("X-Amz-Algorithm") {
        let method_str = method.as_str().to_string();
        let path_str = uri.path().to_string();
        let raw_query = uri.query().unwrap_or("").to_string();

        let mut headers_map = BTreeMap::new();
        for (name, value) in request.headers().iter() {
            if let Ok(v) = value.to_str() {
                headers_map.insert(name.as_str().to_lowercase(), v.to_string());
            }
        }

        match verify_presigned_url(&state, &method_str, &path_str, &raw_query, &headers_map) {
            Ok(()) => return next.run(request).await,
            Err(e) => return e.into_response(),
        }
    }

    // If no Authorization header is present, check anonymous access
    if !request.headers().contains_key("authorization") {
        // Global anonymous mode bypasses auth entirely
        if state.config.anonymous_global {
            return next.run(request).await;
        }

        // Per-bucket anonymous read: only allow read-only operations
        if let Some(ref op) = operation {
            if op.is_read_only() {
                if let Some(bucket_name) = op.bucket() {
                    if let Ok(bucket_meta) = state.metadata.get_bucket(bucket_name) {
                        if bucket_meta.anonymous_read {
                            return next.run(request).await;
                        }
                    }
                }
            }
        }

        // Per-object public access on private buckets
        if let Some(ref op) = operation {
            match op {
                S3Operation::GetObject { bucket, key }
                | S3Operation::HeadObject { bucket, key }
                | S3Operation::GetObjectTagging { bucket, key }
                | S3Operation::GetObjectAcl { bucket, key } => {
                    if let Ok(meta) = state.metadata.get_object_meta(bucket, key) {
                        if meta.public {
                            return next.run(request).await;
                        }
                    }
                }
                S3Operation::ListObjectsV2 { bucket } => {
                    if let Ok(bucket_meta) = state.metadata.get_bucket(bucket) {
                        if bucket_meta.anonymous_list_public {
                            let mut request = request;
                            request.extensions_mut().insert(AnonymousPublicListOnly);
                            return next.run(request).await;
                        }
                    }
                }
                _ => {}
            }
        }
    }

    // Get Authorization header
    let auth_header = match request.headers().get("authorization") {
        Some(val) => match val.to_str() {
            Ok(s) => s.to_string(),
            Err(_) => {
                tracing::debug!("Auth failed: authorization header is not valid UTF-8");
                return simples3_core::S3Error::AccessDenied.into_response();
            }
        },
        None => {
            tracing::debug!(method = %method, path = %path, "Auth failed: no authorization header");
            return simples3_core::S3Error::AccessDenied.into_response();
        }
    };

    // Parse SigV4
    let auth = match sigv4::parse_auth_header(&auth_header) {
        Ok(a) => a,
        Err(e) => {
            tracing::debug!(auth_header = %auth_header, "Auth failed: could not parse SigV4 auth header");
            return e.into_response();
        }
    };

    // Look up credential
    let credential = match state.metadata.get_credential(&auth.access_key_id) {
        Ok(c) => c,
        Err(e) => {
            tracing::debug!(access_key_id = %auth.access_key_id, "Auth failed: credential not found");
            return e.into_response();
        }
    };

    if !credential.active {
        tracing::debug!(access_key_id = %auth.access_key_id, "Auth failed: credential is revoked");
        return simples3_core::S3Error::AccessDenied.into_response();
    }

    // Build headers map for verification
    let mut headers_map = BTreeMap::new();
    for name in &auth.signed_headers {
        if let Some(val) = request.headers().get(name.as_str()) {
            if let Ok(v) = val.to_str() {
                headers_map.insert(name.clone(), v.to_string());
            }
        }
    }

    // Get payload hash
    let payload_hash = request
        .headers()
        .get("x-amz-content-sha256")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("UNSIGNED-PAYLOAD")
        .to_string();

    // Build canonical query string from raw URI query (sorted)
    // We use the raw query string to preserve the exact encoding the client used,
    // since AWS SigV4 requires unreserved chars (A-Z, a-z, 0-9, -, _, ., ~) to NOT be encoded.
    let mut raw_pairs: Vec<(&str, &str)> = uri
        .query()
        .unwrap_or("")
        .split('&')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let mut kv = p.splitn(2, '=');
            let k = kv.next().unwrap_or("");
            let v = kv.next().unwrap_or("");
            (k, v)
        })
        .collect();
    raw_pairs.sort_by(|a, b| a.0.cmp(&b.0));
    let canonical_query: String = raw_pairs
        .iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join("&");

    // Verify signature
    match sigv4::verify_signature(
        method.as_str(),
        uri.path(),
        &canonical_query,
        &headers_map,
        &auth,
        &credential.secret_access_key,
        &payload_hash,
    ) {
        Ok(()) => next.run(request).await,
        Err(e) => {
            tracing::debug!(
                method = %method,
                path = %path,
                access_key_id = %auth.access_key_id,
                signed_headers = ?auth.signed_headers,
                payload_hash = %payload_hash,
                canonical_query = %canonical_query,
                "Auth failed: signature mismatch"
            );
            e.into_response()
        }
    }
}

fn verify_presigned_url(
    state: &AppState,
    method: &str,
    path: &str,
    raw_query: &str,
    headers: &BTreeMap<String, String>,
) -> Result<(), simples3_core::S3Error> {
    // Parse query params from raw query (preserving encoding)
    let query_pairs: Vec<(String, String)> = raw_query
        .split('&')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let mut kv = p.splitn(2, '=');
            let k = kv.next().unwrap_or("").to_string();
            let v = kv.next().unwrap_or("").to_string();
            (k, v)
        })
        .collect();

    let get_param = |name: &str| -> Option<String> {
        query_pairs.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone())
    };

    let algorithm = get_param("X-Amz-Algorithm")
        .ok_or(simples3_core::S3Error::AccessDenied)?;
    if algorithm != "AWS4-HMAC-SHA256" {
        return Err(simples3_core::S3Error::AccessDenied);
    }

    let credential_raw = get_param("X-Amz-Credential")
        .ok_or(simples3_core::S3Error::AccessDenied)?;
    let amz_date = get_param("X-Amz-Date")
        .ok_or(simples3_core::S3Error::AccessDenied)?;
    let expires_str = get_param("X-Amz-Expires")
        .ok_or(simples3_core::S3Error::AccessDenied)?;
    let signed_headers_str = get_param("X-Amz-SignedHeaders")
        .ok_or(simples3_core::S3Error::AccessDenied)?;
    let signature = get_param("X-Amz-Signature")
        .ok_or(simples3_core::S3Error::AccessDenied)?;

    // Percent-decode credential (contains %2F for /)
    let credential = percent_encoding::percent_decode_str(&credential_raw)
        .decode_utf8_lossy()
        .into_owned();

    // Parse credential: AKID/YYYYMMDD/region/s3/aws4_request
    let cred_parts: Vec<&str> = credential.split('/').collect();
    if cred_parts.len() != 5 {
        return Err(simples3_core::S3Error::AccessDenied);
    }
    let access_key_id = cred_parts[0];
    let date = cred_parts[1];
    let region = cred_parts[2];

    // Look up credential
    let cred_record = state.metadata.get_credential(access_key_id)?;
    if !cred_record.active {
        return Err(simples3_core::S3Error::AccessDenied);
    }

    // Check expiration
    let expires: i64 = expires_str.parse().map_err(|_| simples3_core::S3Error::AccessDenied)?;
    // Parse amz_date: 20130524T000000Z
    let amz_date_decoded = percent_encoding::percent_decode_str(&amz_date)
        .decode_utf8_lossy()
        .into_owned();
    let request_time = NaiveDateTime::parse_from_str(&amz_date_decoded, "%Y%m%dT%H%M%SZ")
        .map_err(|_| simples3_core::S3Error::AccessDenied)?;
    let request_time = request_time.and_utc();
    let now = Utc::now();
    let elapsed = (now - request_time).num_seconds();
    if elapsed > expires || elapsed < 0 {
        return Err(simples3_core::S3Error::AccessDenied);
    }

    // Build canonical query string: all query params except X-Amz-Signature, sorted
    let mut canonical_pairs: Vec<(String, String)> = query_pairs
        .iter()
        .filter(|(k, _)| k != "X-Amz-Signature")
        .cloned()
        .collect();
    canonical_pairs.sort_by(|a, b| a.0.cmp(&b.0));
    let canonical_query: String = canonical_pairs
        .iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join("&");

    // Parse signed headers
    let signed_headers_decoded = percent_encoding::percent_decode_str(&signed_headers_str)
        .decode_utf8_lossy()
        .into_owned();
    let signed_headers: Vec<String> = signed_headers_decoded.split(';').map(|s| s.to_string()).collect();

    sigv4::verify_presigned_signature(
        method,
        path,
        &canonical_query,
        headers,
        &signed_headers,
        date,
        &amz_date_decoded,
        region,
        &cred_record.secret_access_key,
        &signature,
    )
}
