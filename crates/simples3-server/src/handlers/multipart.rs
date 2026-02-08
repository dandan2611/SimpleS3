use crate::AppState;
use axum::body::Body;
use axum::extract::Request;
use axum::response::{IntoResponse, Response};
use chrono::Utc;
use http::StatusCode;
use simples3_core::s3::types::{CompletedPart, MultipartUpload, ObjectMeta, PartInfo};
use simples3_core::s3::xml;
use std::sync::Arc;
use uuid::Uuid;

pub async fn create_multipart_upload(
    state: Arc<AppState>,
    bucket: &str,
    key: &str,
) -> Response<Body> {
    if let Err(e) = state.metadata.get_bucket(bucket) {
        return e.into_response();
    }

    let upload_id = Uuid::new_v4().to_string();
    let upload = MultipartUpload {
        upload_id: upload_id.clone(),
        bucket: bucket.to_string(),
        key: key.to_string(),
        created: Utc::now(),
        parts: vec![],
    };

    if let Err(e) = state.metadata.create_multipart_upload(&upload) {
        return e.into_response();
    }

    let body = xml::initiate_multipart_upload_xml(bucket, key, &upload_id);
    (
        StatusCode::OK,
        [("content-type", "application/xml")],
        body,
    )
        .into_response()
}

pub async fn upload_part(
    state: Arc<AppState>,
    _bucket: &str,
    _key: &str,
    upload_id: &str,
    part_number: u32,
    request: Request<Body>,
) -> Response<Body> {
    // Verify upload exists
    let _ = match state.metadata.get_multipart_upload(upload_id) {
        Ok(u) => u,
        Err(e) => return e.into_response(),
    };

    let body_bytes = match axum::body::to_bytes(request.into_body(), usize::MAX).await {
        Ok(b) => b,
        Err(e) => {
            return simples3_core::S3Error::InternalError(e.to_string()).into_response();
        }
    };

    let (size, etag) = match state
        .filestore
        .write_part(upload_id, part_number, &body_bytes)
        .await
    {
        Ok(r) => r,
        Err(e) => return e.into_response(),
    };

    let part_info = PartInfo {
        part_number,
        etag: etag.clone(),
        size,
        last_modified: Utc::now(),
    };

    if let Err(e) = state.metadata.add_part_to_upload(upload_id, part_info) {
        return e.into_response();
    }

    (StatusCode::OK, [("etag", format!("\"{}\"", etag).as_str())], "").into_response()
}

pub async fn complete_multipart_upload(
    state: Arc<AppState>,
    bucket: &str,
    key: &str,
    upload_id: &str,
    request: Request<Body>,
) -> Response<Body> {
    let _upload = match state.metadata.get_multipart_upload(upload_id) {
        Ok(u) => u,
        Err(e) => return e.into_response(),
    };

    // Parse the XML body to get part list
    let body_bytes = match axum::body::to_bytes(request.into_body(), usize::MAX).await {
        Ok(b) => b,
        Err(e) => {
            return simples3_core::S3Error::InternalError(e.to_string()).into_response();
        }
    };

    let parts = match parse_complete_multipart_xml(&body_bytes) {
        Ok(p) => p,
        Err(e) => return e.into_response(),
    };

    // Validate parts are in order
    for i in 1..parts.len() {
        if parts[i].part_number <= parts[i - 1].part_number {
            return simples3_core::S3Error::InvalidPartOrder.into_response();
        }
    }

    let part_numbers: Vec<u32> = parts.iter().map(|p| p.part_number).collect();

    let (size, etag) = match state
        .filestore
        .assemble_parts(bucket, key, upload_id, &part_numbers)
        .await
    {
        Ok(r) => r,
        Err(e) => return e.into_response(),
    };

    // Store object metadata
    let content_type = "application/octet-stream".to_string();
    let meta = ObjectMeta {
        bucket: bucket.to_string(),
        key: key.to_string(),
        size,
        etag: etag.clone(),
        content_type,
        last_modified: Utc::now(),
        public: false,
    };

    if let Err(e) = state.metadata.put_object_meta(&meta) {
        return e.into_response();
    }

    // Cleanup
    let _ = state.filestore.cleanup_multipart(upload_id).await;
    let _ = state.metadata.delete_multipart_upload(upload_id);

    let location = format!("http://{}/{}/{}", state.config.hostname, bucket, key);
    let body = xml::complete_multipart_upload_xml(bucket, key, &etag, &location);
    (
        StatusCode::OK,
        [("content-type", "application/xml")],
        body,
    )
        .into_response()
}

pub async fn abort_multipart_upload(
    state: Arc<AppState>,
    upload_id: &str,
) -> Response<Body> {
    if let Err(e) = state.metadata.get_multipart_upload(upload_id) {
        return e.into_response();
    }

    let _ = state.filestore.cleanup_multipart(upload_id).await;
    let _ = state.metadata.delete_multipart_upload(upload_id);

    StatusCode::NO_CONTENT.into_response()
}

pub async fn list_parts(state: Arc<AppState>, upload_id: &str) -> Response<Body> {
    let upload = match state.metadata.get_multipart_upload(upload_id) {
        Ok(u) => u,
        Err(e) => return e.into_response(),
    };

    let body = xml::list_parts_xml(&upload);
    (
        StatusCode::OK,
        [("content-type", "application/xml")],
        body,
    )
        .into_response()
}

fn parse_complete_multipart_xml(data: &[u8]) -> Result<Vec<CompletedPart>, simples3_core::S3Error> {
    use quick_xml::Reader;
    use quick_xml::events::Event;

    let mut reader = Reader::from_reader(data);
    let mut parts = Vec::new();
    let mut current_part_number: Option<u32> = None;
    let mut current_etag: Option<String> = None;
    let mut in_part = false;
    let mut current_element = String::new();

    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match name.as_str() {
                    "Part" => {
                        in_part = true;
                        current_part_number = None;
                        current_etag = None;
                    }
                    _ if in_part => {
                        current_element = name;
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(ref e)) => {
                if in_part {
                    let text = e.unescape().unwrap_or_default().to_string();
                    match current_element.as_str() {
                        "PartNumber" => {
                            current_part_number = text.parse().ok();
                        }
                        "ETag" => {
                            current_etag = Some(text.trim_matches('"').to_string());
                        }
                        _ => {}
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if name == "Part" {
                    in_part = false;
                    if let (Some(pn), Some(etag)) = (current_part_number, current_etag.take()) {
                        parts.push(CompletedPart {
                            part_number: pn,
                            etag,
                        });
                    }
                }
                current_element.clear();
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(simples3_core::S3Error::InvalidArgument(format!(
                    "Invalid XML: {}",
                    e
                )));
            }
            _ => {}
        }
        buf.clear();
    }

    Ok(parts)
}
