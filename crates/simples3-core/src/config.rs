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
        }
    }
}
