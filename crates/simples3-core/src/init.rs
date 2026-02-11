use crate::error::S3Error;
use crate::storage::MetadataStore;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct InitConfig {
    #[serde(default)]
    pub buckets: Vec<InitBucket>,
    #[serde(default)]
    pub credentials: Vec<InitCredential>,
}

#[derive(Debug, Deserialize)]
pub struct InitBucket {
    pub name: String,
    #[serde(default)]
    pub anonymous_read: bool,
    #[serde(default)]
    pub anonymous_list_public: bool,
    #[serde(default)]
    pub cors_origins: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct InitCredential {
    pub access_key_id: String,
    pub secret_access_key: String,
    #[serde(default)]
    pub description: String,
}

pub fn load(path: &Path) -> Result<InitConfig, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read init config file '{}': {}", path.display(), e))?;
    toml::from_str(&content)
        .map_err(|e| format!("Failed to parse init config file '{}': {}", path.display(), e))
}

pub fn apply(config: &InitConfig, metadata: &MetadataStore) -> Result<(), String> {
    for bucket in &config.buckets {
        match metadata.create_bucket(&bucket.name) {
            Ok(_) => {
                tracing::info!(bucket = %bucket.name, "Init: created bucket");
            }
            Err(S3Error::BucketAlreadyExists) => {
                tracing::debug!(bucket = %bucket.name, "Init: bucket already exists, skipping");
            }
            Err(e) => {
                return Err(format!("Failed to create bucket '{}': {}", bucket.name, e));
            }
        }
        if bucket.anonymous_read {
            metadata
                .set_bucket_anonymous_read(&bucket.name, true)
                .map_err(|e| {
                    format!(
                        "Failed to set anonymous read on bucket '{}': {}",
                        bucket.name, e
                    )
                })?;
            tracing::info!(bucket = %bucket.name, "Init: enabled anonymous read");
        }
        if bucket.anonymous_list_public {
            metadata
                .set_bucket_anonymous_list_public(&bucket.name, true)
                .map_err(|e| {
                    format!(
                        "Failed to set anonymous list public on bucket '{}': {}",
                        bucket.name, e
                    )
                })?;
            tracing::info!(bucket = %bucket.name, "Init: enabled anonymous list public");
        }
        if let Some(ref origins) = bucket.cors_origins {
            use crate::s3::types::{CorsConfiguration, CorsRule};
            let cors_config = CorsConfiguration {
                rules: vec![CorsRule {
                    id: Some("init-cors".into()),
                    allowed_origins: origins.clone(),
                    allowed_methods: vec![
                        "GET".into(), "PUT".into(), "POST".into(),
                        "DELETE".into(), "HEAD".into(),
                    ],
                    allowed_headers: vec!["*".into()],
                    expose_headers: vec![],
                    max_age_seconds: None,
                }],
            };
            metadata
                .put_cors_configuration(&bucket.name, &cors_config)
                .map_err(|e| {
                    format!(
                        "Failed to set CORS on bucket '{}': {}",
                        bucket.name, e
                    )
                })?;
            tracing::info!(bucket = %bucket.name, "Init: configured CORS");
        }
    }

    for cred in &config.credentials {
        match metadata.create_credential(
            &cred.access_key_id,
            &cred.secret_access_key,
            &cred.description,
        ) {
            Ok(_) => {
                tracing::info!(access_key_id = %cred.access_key_id, "Init: created credential");
            }
            Err(S3Error::InvalidArgument(_)) => {
                tracing::debug!(access_key_id = %cred.access_key_id, "Init: credential already exists, skipping");
            }
            Err(e) => {
                return Err(format!(
                    "Failed to create credential '{}': {}",
                    cred.access_key_id, e
                ));
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store() -> (MetadataStore, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let store = MetadataStore::open(dir.path()).unwrap();
        (store, dir)
    }

    #[test]
    fn test_load_valid_toml() {
        let toml_str = r#"
[[buckets]]
name = "my-bucket"

[[buckets]]
name = "public-assets"
anonymous_read = true

[[credentials]]
access_key_id = "AKID_CI"
secret_access_key = "secret123"
description = "CI pipeline"

[[credentials]]
access_key_id = "AKID_DEV"
secret_access_key = "devkey456"
description = "Development"
"#;
        let config: InitConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.buckets.len(), 2);
        assert_eq!(config.buckets[0].name, "my-bucket");
        assert!(!config.buckets[0].anonymous_read);
        assert_eq!(config.buckets[1].name, "public-assets");
        assert!(config.buckets[1].anonymous_read);
        assert_eq!(config.credentials.len(), 2);
        assert_eq!(config.credentials[0].access_key_id, "AKID_CI");
        assert_eq!(config.credentials[0].secret_access_key, "secret123");
        assert_eq!(config.credentials[0].description, "CI pipeline");
    }

    #[test]
    fn test_load_empty_sections() {
        let toml_str = "";
        let config: InitConfig = toml::from_str(toml_str).unwrap();
        assert!(config.buckets.is_empty());
        assert!(config.credentials.is_empty());
    }

    #[test]
    fn test_apply_creates_buckets_and_credentials() {
        let (store, _dir) = temp_store();
        let config = InitConfig {
            buckets: vec![
                InitBucket {
                    name: "bucket-a".into(),
                    anonymous_read: false,
                    anonymous_list_public: false,
                    cors_origins: None,
                },
                InitBucket {
                    name: "bucket-b".into(),
                    anonymous_read: false,
                    anonymous_list_public: false,
                    cors_origins: None,
                },
            ],
            credentials: vec![InitCredential {
                access_key_id: "AKID1".into(),
                secret_access_key: "SECRET1".into(),
                description: "test".into(),
            }],
        };
        apply(&config, &store).unwrap();

        let buckets = store.list_buckets().unwrap();
        assert_eq!(buckets.len(), 2);

        let cred = store.get_credential("AKID1").unwrap();
        assert_eq!(cred.secret_access_key, "SECRET1");
    }

    #[test]
    fn test_apply_idempotent() {
        let (store, _dir) = temp_store();
        let config = InitConfig {
            buckets: vec![InitBucket {
                name: "idem-bucket".into(),
                anonymous_read: false,
                anonymous_list_public: false,
                cors_origins: None,
            }],
            credentials: vec![InitCredential {
                access_key_id: "AKID_IDEM".into(),
                secret_access_key: "SECRET".into(),
                description: "idem".into(),
            }],
        };
        apply(&config, &store).unwrap();
        // Second apply should succeed without error
        apply(&config, &store).unwrap();

        let buckets = store.list_buckets().unwrap();
        assert_eq!(buckets.len(), 1);
        let creds = store.list_credentials().unwrap();
        assert_eq!(creds.len(), 1);
    }

    #[test]
    fn test_apply_anonymous_read() {
        let (store, _dir) = temp_store();
        let config = InitConfig {
            buckets: vec![InitBucket {
                name: "public".into(),
                anonymous_read: true,
                anonymous_list_public: false,
                cors_origins: None,
            }],
            credentials: vec![],
        };
        apply(&config, &store).unwrap();

        let bucket = store.get_bucket("public").unwrap();
        assert!(bucket.anonymous_read);
    }

    #[test]
    fn test_apply_cors_origins() {
        let (store, _dir) = temp_store();
        let config = InitConfig {
            buckets: vec![InitBucket {
                name: "cors-bkt".into(),
                anonymous_read: false,
                anonymous_list_public: false,
                cors_origins: Some(vec!["https://example.com".into()]),
            }],
            credentials: vec![],
        };
        apply(&config, &store).unwrap();

        let cors = store.get_cors_configuration("cors-bkt").unwrap();
        assert_eq!(cors.rules.len(), 1);
        assert_eq!(cors.rules[0].allowed_origins, vec!["https://example.com"]);
    }
}
