use crate::AppState;
use axum::body::Body;
use axum::extract::Request;
use axum::response::{IntoResponse, Response};
use chrono::Utc;
use http::StatusCode;
use quick_xml::Reader;
use quick_xml::events::Event;
use simples3_core::s3::types::{ListObjectsV2Request, ObjectMeta};
use simples3_core::s3::xml;
use std::collections::HashMap;
use std::sync::Arc;
use tokio_util::io::ReaderStream;

pub async fn put_object(
    state: Arc<AppState>,
    bucket: &str,
    key: &str,
    request: Request<Body>,
) -> Response<Body> {
    // Verify bucket exists
    if let Err(e) = state.metadata.get_bucket(bucket) {
        return e.into_response();
    }

    let content_type = request
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string();

    // Stream body to disk
    let body_bytes = match axum::body::to_bytes(request.into_body(), usize::MAX).await {
        Ok(b) => b,
        Err(e) => {
            return simples3_core::S3Error::InternalError(e.to_string()).into_response();
        }
    };

    let (size, etag) = match state.filestore.write_object(bucket, key, &body_bytes).await {
        Ok(r) => r,
        Err(e) => return e.into_response(),
    };

    let meta = ObjectMeta {
        bucket: bucket.to_string(),
        key: key.to_string(),
        size,
        etag: etag.clone(),
        content_type,
        last_modified: Utc::now(),
    };

    if let Err(e) = state.metadata.put_object_meta(&meta) {
        return e.into_response();
    }

    (StatusCode::OK, [("etag", format!("\"{}\"", etag).as_str())], "").into_response()
}

pub async fn get_object(state: Arc<AppState>, bucket: &str, key: &str) -> Response<Body> {
    let meta = match state.metadata.get_object_meta(bucket, key) {
        Ok(m) => m,
        Err(e) => return e.into_response(),
    };

    let file_path = state.filestore.open_object_file(bucket, key);
    let file = match tokio::fs::File::open(&file_path).await {
        Ok(f) => f,
        Err(_) => return simples3_core::S3Error::NoSuchKey.into_response(),
    };

    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    let mut builder = Response::builder()
        .status(StatusCode::OK)
        .header("content-type", &meta.content_type)
        .header("content-length", meta.size.to_string())
        .header("etag", format!("\"{}\"", meta.etag))
        .header("last-modified", meta.last_modified.to_rfc2822());

    if let Ok(tags) = state.metadata.get_object_tagging(bucket, key) {
        if !tags.is_empty() {
            builder = builder.header("x-amz-tagging-count", tags.len().to_string());
        }
    }

    builder.body(body).unwrap()
}

pub async fn head_object(state: Arc<AppState>, bucket: &str, key: &str) -> Response<Body> {
    let meta = match state.metadata.get_object_meta(bucket, key) {
        Ok(m) => m,
        Err(e) => return e.into_response(),
    };

    let mut builder = Response::builder()
        .status(StatusCode::OK)
        .header("content-type", &meta.content_type)
        .header("content-length", meta.size.to_string())
        .header("etag", format!("\"{}\"", meta.etag))
        .header("last-modified", meta.last_modified.to_rfc2822());

    if let Ok(tags) = state.metadata.get_object_tagging(bucket, key) {
        if !tags.is_empty() {
            builder = builder.header("x-amz-tagging-count", tags.len().to_string());
        }
    }

    builder.body(Body::empty()).unwrap()
}

pub async fn delete_object(state: Arc<AppState>, bucket: &str, key: &str) -> Response<Body> {
    if let Err(e) = state.metadata.delete_object_meta(bucket, key) {
        return e.into_response();
    }
    if let Err(e) = state.filestore.delete_object(bucket, key).await {
        return e.into_response();
    }
    StatusCode::NO_CONTENT.into_response()
}

pub async fn list_objects_v2(
    state: Arc<AppState>,
    bucket: &str,
    query: &HashMap<String, String>,
) -> Response<Body> {
    // Verify bucket exists
    if let Err(e) = state.metadata.get_bucket(bucket) {
        return e.into_response();
    }

    let prefix = query.get("prefix").cloned().unwrap_or_default();
    let delimiter = query.get("delimiter").cloned().unwrap_or_default();
    let max_keys: u32 = query
        .get("max-keys")
        .and_then(|v| v.parse().ok())
        .unwrap_or(1000);
    let continuation_token = query.get("continuation-token").cloned();
    let start_after = query.get("start-after").cloned();

    let req = ListObjectsV2Request {
        bucket: bucket.to_string(),
        prefix,
        delimiter,
        max_keys,
        continuation_token,
        start_after,
    };

    match state.metadata.list_objects_v2(&req) {
        Ok(resp) => {
            let body = xml::list_objects_v2_xml(&resp);
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

// --- Tagging handlers ---

fn parse_tagging_xml(data: &[u8]) -> Result<HashMap<String, String>, simples3_core::S3Error> {
    let mut reader = Reader::from_reader(data);
    reader.config_mut().trim_text(true);
    let mut tags = HashMap::new();
    let mut buf = Vec::new();
    let mut current_key = String::new();
    let mut current_value = String::new();
    let mut in_key = false;
    let mut in_value = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => match e.name().as_ref() {
                b"Key" => in_key = true,
                b"Value" => in_value = true,
                _ => {}
            },
            Ok(Event::Text(e)) => {
                let text = e.unescape().map_err(|e| simples3_core::S3Error::InvalidArgument(e.to_string()))?.into_owned();
                if in_key {
                    current_key = text;
                } else if in_value {
                    current_value = text;
                }
            }
            Ok(Event::End(e)) => match e.name().as_ref() {
                b"Key" => in_key = false,
                b"Value" => in_value = false,
                b"Tag" => {
                    if !current_key.is_empty() {
                        tags.insert(current_key.clone(), current_value.clone());
                    }
                    current_key.clear();
                    current_value.clear();
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(e) => return Err(simples3_core::S3Error::InvalidArgument(e.to_string())),
            _ => {}
        }
        buf.clear();
    }
    Ok(tags)
}

pub async fn put_object_tagging(
    state: Arc<AppState>,
    bucket: &str,
    key: &str,
    request: Request<Body>,
) -> Response<Body> {
    let body_bytes = match axum::body::to_bytes(request.into_body(), usize::MAX).await {
        Ok(b) => b,
        Err(e) => return simples3_core::S3Error::InternalError(e.to_string()).into_response(),
    };

    let tags = match parse_tagging_xml(&body_bytes) {
        Ok(t) => t,
        Err(e) => return e.into_response(),
    };

    if let Err(e) = state.metadata.put_object_tagging(bucket, key, &tags) {
        return e.into_response();
    }

    StatusCode::OK.into_response()
}

pub async fn get_object_tagging(
    state: Arc<AppState>,
    bucket: &str,
    key: &str,
) -> Response<Body> {
    match state.metadata.get_object_tagging(bucket, key) {
        Ok(tags) => {
            let body = xml::get_tagging_xml(&tags);
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

pub async fn delete_object_tagging(
    state: Arc<AppState>,
    bucket: &str,
    key: &str,
) -> Response<Body> {
    if let Err(e) = state.metadata.delete_object_tagging(bucket, key) {
        return e.into_response();
    }
    StatusCode::NO_CONTENT.into_response()
}

// --- CopyObject handler ---

pub async fn copy_object(
    state: Arc<AppState>,
    dest_bucket: &str,
    dest_key: &str,
    request: Request<Body>,
) -> Response<Body> {
    let copy_source = match request.headers().get("x-amz-copy-source") {
        Some(v) => match v.to_str() {
            Ok(s) => s.to_string(),
            Err(_) => return simples3_core::S3Error::InvalidArgument("Invalid x-amz-copy-source".into()).into_response(),
        },
        None => return simples3_core::S3Error::InvalidArgument("Missing x-amz-copy-source".into()).into_response(),
    };

    // Strip leading '/' and URL-decode
    let copy_source = copy_source.trim_start_matches('/');
    let copy_source = percent_encoding::percent_decode_str(copy_source)
        .decode_utf8_lossy()
        .into_owned();

    let (src_bucket, src_key) = match copy_source.find('/') {
        Some(idx) => (&copy_source[..idx], &copy_source[idx + 1..]),
        None => return simples3_core::S3Error::InvalidArgument("Invalid x-amz-copy-source format".into()).into_response(),
    };

    if src_key.is_empty() {
        return simples3_core::S3Error::InvalidArgument("Source key is empty".into()).into_response();
    }

    // Verify source and dest buckets exist
    if let Err(e) = state.metadata.get_bucket(src_bucket) {
        return e.into_response();
    }
    if let Err(e) = state.metadata.get_bucket(dest_bucket) {
        return e.into_response();
    }

    // Get source metadata
    let src_meta = match state.metadata.get_object_meta(src_bucket, src_key) {
        Ok(m) => m,
        Err(e) => return e.into_response(),
    };

    // Read source data and write to destination
    let data = match state.filestore.read_object(src_bucket, src_key).await {
        Ok(d) => d,
        Err(e) => return e.into_response(),
    };

    let (size, etag) = match state.filestore.write_object(dest_bucket, dest_key, &data).await {
        Ok(r) => r,
        Err(e) => return e.into_response(),
    };

    let now = Utc::now();
    let dest_meta = ObjectMeta {
        bucket: dest_bucket.to_string(),
        key: dest_key.to_string(),
        size,
        etag: etag.clone(),
        content_type: src_meta.content_type,
        last_modified: now,
    };

    if let Err(e) = state.metadata.put_object_meta(&dest_meta) {
        return e.into_response();
    }

    // Copy tags from source to destination
    if let Ok(tags) = state.metadata.get_object_tagging(src_bucket, src_key) {
        if !tags.is_empty() {
            let _ = state.metadata.put_object_tagging(dest_bucket, dest_key, &tags);
        }
    }

    let body = xml::copy_object_result_xml(&etag, &now);
    (
        StatusCode::OK,
        [("content-type", "application/xml")],
        body,
    )
        .into_response()
}

// --- DeleteObjects (batch delete) handler ---

fn parse_delete_objects_xml(data: &[u8]) -> Result<(Vec<String>, bool), simples3_core::S3Error> {
    let mut reader = Reader::from_reader(data);
    reader.config_mut().trim_text(true);
    let mut keys = Vec::new();
    let mut quiet = false;
    let mut buf = Vec::new();
    let mut in_key = false;
    let mut in_quiet = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => match e.name().as_ref() {
                b"Key" => in_key = true,
                b"Quiet" => in_quiet = true,
                _ => {}
            },
            Ok(Event::Text(e)) => {
                let text = e.unescape().map_err(|e| simples3_core::S3Error::InvalidArgument(e.to_string()))?.into_owned();
                if in_key {
                    keys.push(text);
                } else if in_quiet {
                    quiet = text == "true";
                }
            }
            Ok(Event::End(e)) => match e.name().as_ref() {
                b"Key" => in_key = false,
                b"Quiet" => in_quiet = false,
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(e) => return Err(simples3_core::S3Error::InvalidArgument(e.to_string())),
            _ => {}
        }
        buf.clear();
    }
    Ok((keys, quiet))
}

pub async fn delete_objects(
    state: Arc<AppState>,
    bucket: &str,
    request: Request<Body>,
) -> Response<Body> {
    // Verify bucket exists
    if let Err(e) = state.metadata.get_bucket(bucket) {
        return e.into_response();
    }

    let body_bytes = match axum::body::to_bytes(request.into_body(), usize::MAX).await {
        Ok(b) => b,
        Err(e) => return simples3_core::S3Error::InternalError(e.to_string()).into_response(),
    };

    let (keys, quiet) = match parse_delete_objects_xml(&body_bytes) {
        Ok(r) => r,
        Err(e) => return e.into_response(),
    };

    let mut deleted = Vec::new();
    let mut errors: Vec<(String, String, String)> = Vec::new();

    for key in keys {
        // Delete meta (which also cleans up tags)
        match state.metadata.delete_object_meta(bucket, &key) {
            Ok(()) => {}
            Err(simples3_core::S3Error::NoSuchKey) => {
                // AWS treats deleting nonexistent keys as success
            }
            Err(e) => {
                errors.push((key.clone(), e.code().to_string(), e.to_string()));
                continue;
            }
        }
        // Delete file
        if let Err(e) = state.filestore.delete_object(bucket, &key).await {
            errors.push((key.clone(), e.code().to_string(), e.to_string()));
            continue;
        }
        deleted.push(key);
    }

    let body = xml::delete_objects_result_xml(&deleted, &errors, quiet);
    (
        StatusCode::OK,
        [("content-type", "application/xml")],
        body,
    )
        .into_response()
}
