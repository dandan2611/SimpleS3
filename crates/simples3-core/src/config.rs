use std::env;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    pub bind: String,
    pub data_dir: PathBuf,
    pub metadata_dir: PathBuf,
    pub hostname: String,
    pub region: String,
    pub log_level: String,
    pub anonymous_global: bool,
    pub admin_enabled: bool,
    pub admin_bind: String,
    pub admin_token: Option<String>,
    pub multipart_ttl_secs: u64,
    pub multipart_cleanup_interval_secs: u64,
    pub lifecycle_scan_interval_secs: u64,
    pub cors_origins: Option<Vec<String>>,
    pub max_object_size: usize,
    pub max_xml_body_size: usize,
    pub max_policy_body_size: usize,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            bind: env::var("SIMPLES3_BIND").unwrap_or_else(|_| "0.0.0.0:9000".into()),
            data_dir: PathBuf::from(env::var("SIMPLES3_DATA_DIR").unwrap_or_else(|_| "./data".into())),
            metadata_dir: PathBuf::from(
                env::var("SIMPLES3_METADATA_DIR").unwrap_or_else(|_| "./metadata".into()),
            ),
            hostname: env::var("SIMPLES3_HOSTNAME").unwrap_or_else(|_| "s3.localhost".into()),
            region: env::var("SIMPLES3_REGION").unwrap_or_else(|_| "us-east-1".into()),
            log_level: env::var("SIMPLES3_LOG_LEVEL").unwrap_or_else(|_| "info".into()),
            anonymous_global: env::var("SIMPLES3_ANONYMOUS_GLOBAL")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(false),
            admin_enabled: env::var("SIMPLES3_ADMIN_ENABLED")
                .map(|v| v != "false" && v != "0")
                .unwrap_or(true),
            admin_bind: env::var("SIMPLES3_ADMIN_BIND")
                .unwrap_or_else(|_| "127.0.0.1:9001".into()),
            admin_token: env::var("SIMPLES3_ADMIN_TOKEN")
                .ok()
                .filter(|s| !s.is_empty()),
            multipart_ttl_secs: env::var("SIMPLES3_MULTIPART_TTL")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(86400),
            multipart_cleanup_interval_secs: env::var("SIMPLES3_MULTIPART_CLEANUP_INTERVAL")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3600),
            lifecycle_scan_interval_secs: env::var("SIMPLES3_LIFECYCLE_SCAN_INTERVAL")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3600),
            cors_origins: env::var("SIMPLES3_CORS_ORIGINS")
                .ok()
                .filter(|s| !s.is_empty())
                .map(|s| s.split(',').map(|o| o.trim().to_string()).collect()),
            max_object_size: env::var("SIMPLES3_MAX_OBJECT_SIZE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(5 * 1024 * 1024 * 1024),
            max_xml_body_size: env::var("SIMPLES3_MAX_XML_BODY_SIZE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(256 * 1024),
            max_policy_body_size: env::var("SIMPLES3_MAX_POLICY_BODY_SIZE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(20 * 1024),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bind: "0.0.0.0:9000".into(),
            data_dir: PathBuf::from("./data"),
            metadata_dir: PathBuf::from("./metadata"),
            hostname: "s3.localhost".into(),
            region: "us-east-1".into(),
            log_level: "info".into(),
            anonymous_global: false,
            admin_enabled: true,
            admin_bind: "127.0.0.1:9001".into(),
            admin_token: None,
            multipart_ttl_secs: 86400,
            multipart_cleanup_interval_secs: 3600,
            lifecycle_scan_interval_secs: 3600,
            cors_origins: None,
            max_object_size: 5 * 1024 * 1024 * 1024,
            max_xml_body_size: 256 * 1024,
            max_policy_body_size: 20 * 1024,
        }
    }
}
