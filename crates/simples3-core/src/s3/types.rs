use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BucketMeta {
    pub name: String,
    pub creation_date: DateTime<Utc>,
    pub anonymous_read: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectMeta {
    pub bucket: String,
    pub key: String,
    pub size: u64,
    pub etag: String,
    pub content_type: String,
    pub last_modified: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultipartUpload {
    pub upload_id: String,
    pub bucket: String,
    pub key: String,
    pub created: DateTime<Utc>,
    pub parts: Vec<PartInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartInfo {
    pub part_number: u32,
    pub etag: String,
    pub size: u64,
    pub last_modified: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessKeyRecord {
    pub access_key_id: String,
    pub secret_access_key: String,
    pub description: String,
    pub created: DateTime<Utc>,
    pub active: bool,
}

#[derive(Debug, Clone)]
pub struct ListObjectsV2Request {
    pub bucket: String,
    pub prefix: String,
    pub delimiter: String,
    pub max_keys: u32,
    pub continuation_token: Option<String>,
    pub start_after: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ListObjectsV2Response {
    pub name: String,
    pub prefix: String,
    pub delimiter: String,
    pub max_keys: u32,
    pub is_truncated: bool,
    pub contents: Vec<ObjectMeta>,
    pub common_prefixes: Vec<String>,
    pub next_continuation_token: Option<String>,
    pub key_count: u32,
}

#[derive(Debug, Clone)]
pub struct CompletedPart {
    pub part_number: u32,
    pub etag: String,
}
