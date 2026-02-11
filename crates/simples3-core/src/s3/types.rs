use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BucketMeta {
    pub name: String,
    pub creation_date: DateTime<Utc>,
    pub anonymous_read: bool,
    #[serde(default)]
    pub anonymous_list_public: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectMeta {
    pub bucket: String,
    pub key: String,
    pub size: u64,
    pub etag: String,
    pub content_type: String,
    pub last_modified: DateTime<Utc>,
    #[serde(default)]
    pub public: bool,
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

// --- Lifecycle types ---

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LifecycleStatus {
    Enabled,
    Disabled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LifecycleTagFilter {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifecycleRule {
    pub id: String,
    pub prefix: String,
    pub status: LifecycleStatus,
    pub expiration_days: u32,
    #[serde(default)]
    pub expiration_date: Option<String>,
    #[serde(default)]
    pub tags: Vec<LifecycleTagFilter>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifecycleConfiguration {
    pub rules: Vec<LifecycleRule>,
}

// --- Bucket Policy types ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BucketPolicy {
    #[serde(rename = "Version")]
    pub version: String,
    #[serde(rename = "Statement")]
    pub statements: Vec<PolicyStatement>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyStatement {
    #[serde(rename = "Sid", skip_serializing_if = "Option::is_none")]
    pub sid: Option<String>,
    #[serde(rename = "Effect")]
    pub effect: PolicyEffect,
    #[serde(rename = "Principal")]
    pub principal: PolicyPrincipal,
    #[serde(rename = "Action")]
    pub action: OneOrMany<String>,
    #[serde(rename = "Resource")]
    pub resource: OneOrMany<String>,
    #[serde(rename = "Condition", skip_serializing_if = "Option::is_none")]
    pub condition: Option<PolicyCondition>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PolicyEffect {
    Allow,
    Deny,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PolicyPrincipal {
    Wildcard(String),
    Mapped(HashMap<String, OneOrMany<String>>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OneOrMany<T> {
    One(T),
    Many(Vec<T>),
}

impl<T> OneOrMany<T> {
    pub fn as_slice(&self) -> &[T] {
        match self {
            OneOrMany::One(v) => std::slice::from_ref(v),
            OneOrMany::Many(v) => v,
        }
    }
}

pub type PolicyCondition = HashMap<String, HashMap<String, OneOrMany<String>>>;

// --- CORS types ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorsRule {
    #[serde(default)]
    pub id: Option<String>,
    pub allowed_origins: Vec<String>,
    pub allowed_methods: Vec<String>,
    #[serde(default)]
    pub allowed_headers: Vec<String>,
    #[serde(default)]
    pub expose_headers: Vec<String>,
    #[serde(default)]
    pub max_age_seconds: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorsConfiguration {
    pub rules: Vec<CorsRule>,
}
