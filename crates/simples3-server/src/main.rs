use clap::Parser;
use simples3_core::Config;
use simples3_server::{AppState, router};
use std::path::Path;
use std::net::SocketAddr;
use std::sync::Arc;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "simples3-server", about = "Simple S3-compatible object storage server")]
struct Cli {
    /// Address to bind to (overrides SIMPLES3_BIND)
    #[arg(long)]
    bind: Option<String>,

    /// Data directory (overrides SIMPLES3_DATA_DIR)
    #[arg(long)]
    data_dir: Option<String>,

    /// Metadata directory (overrides SIMPLES3_METADATA_DIR)
    #[arg(long)]
    metadata_dir: Option<String>,

    /// Server hostname (overrides SIMPLES3_HOSTNAME)
    #[arg(long)]
    hostname: Option<String>,

    /// S3 region (overrides SIMPLES3_REGION)
    #[arg(long)]
    region: Option<String>,

    /// Admin API bind address (overrides SIMPLES3_ADMIN_BIND)
    #[arg(long)]
    admin_bind: Option<String>,

    /// Path to init config TOML file (overrides SIMPLES3_INIT_CONFIG)
    #[arg(long, env = "SIMPLES3_INIT_CONFIG")]
    init_config: Option<String>,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let mut config = Config::from_env();

    if let Some(bind) = cli.bind {
        config.bind = bind;
    }
    if let Some(data_dir) = cli.data_dir {
        config.data_dir = data_dir.into();
    }
    if let Some(metadata_dir) = cli.metadata_dir {
        config.metadata_dir = metadata_dir.into();
    }
    if let Some(hostname) = cli.hostname {
        config.hostname = hostname;
    }
    if let Some(region) = cli.region {
        config.region = region;
    }
    if let Some(admin_bind) = cli.admin_bind {
        config.admin_bind = admin_bind;
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new(&config.log_level)),
        )
        .init();

    std::fs::create_dir_all(&config.data_dir).expect("Failed to create data directory");
    std::fs::create_dir_all(&config.metadata_dir).expect("Failed to create metadata directory");

    let metadata =
        simples3_core::storage::MetadataStore::open(&config.metadata_dir).expect("Failed to open metadata store");
    let filestore = simples3_core::storage::FileStore::new(&config.data_dir);

    if let Some(ref init_path) = cli.init_config {
        let init_cfg = simples3_core::init::load(Path::new(init_path))
            .expect("Failed to load init config");
        simples3_core::init::apply(&init_cfg, &metadata)
            .expect("Failed to apply init config");
        tracing::info!(path = %init_path, "Init config applied successfully");
    }

    let metrics_handle = simples3_server::metrics::init_metrics();

    let state = Arc::new(AppState {
        config: config.clone(),
        metadata,
        filestore,
        start_time: std::time::Instant::now(),
        metrics_handle,
    });

    let s3_app = router::build_s3_router(state.clone());
    let s3_listener = tokio::net::TcpListener::bind(&config.bind)
        .await
        .expect("Failed to bind S3 listener");
    tracing::info!("simples3 S3 API listening on {}", config.bind);

    let cleanup_handle = tokio::spawn(multipart_cleanup_loop(state.clone()));
    let lifecycle_handle = tokio::spawn(lifecycle_expiration_loop(state.clone()));

    if config.admin_enabled {
        let admin_app = router::build_admin_router(state);
        let admin_listener = tokio::net::TcpListener::bind(&config.admin_bind)
            .await
            .expect("Failed to bind admin listener");
        tracing::info!("simples3 admin API listening on {}", config.admin_bind);

        let s3_handle = tokio::spawn(async move {
            axum::serve(s3_listener, s3_app.into_make_service_with_connect_info::<SocketAddr>())
                .with_graceful_shutdown(shutdown_signal())
                .await
                .expect("S3 server error");
        });

        let admin_handle = tokio::spawn(async move {
            axum::serve(admin_listener, admin_app.into_make_service_with_connect_info::<SocketAddr>())
                .await
                .expect("Admin server error");
        });

        // Wait for S3 server to finish (shutdown signal), then drop admin and cleanup
        let _ = s3_handle.await;
        admin_handle.abort();
        cleanup_handle.abort();
        lifecycle_handle.abort();
    } else {
        tracing::info!("Admin API is disabled");
        axum::serve(s3_listener, s3_app.into_make_service_with_connect_info::<SocketAddr>())
            .with_graceful_shutdown(shutdown_signal())
            .await
            .expect("S3 server error");
        cleanup_handle.abort();
        lifecycle_handle.abort();
    }
}

async fn multipart_cleanup_loop(state: Arc<AppState>) {
    let ttl = state.config.multipart_ttl_secs;
    let interval_secs = state.config.multipart_cleanup_interval_secs;
    if ttl == 0 || interval_secs == 0 {
        tracing::info!("Multipart upload cleanup is disabled (TTL = {ttl}, interval = {interval_secs})");
        return;
    }
    tracing::info!(
        ttl_secs = ttl,
        interval_secs = interval_secs,
        "Starting multipart upload cleanup task"
    );

    let mut interval = tokio::time::interval(std::time::Duration::from_secs(interval_secs));
    // First tick completes immediately — skip it so we don't clean on startup
    interval.tick().await;

    loop {
        interval.tick().await;

        let uploads = match state.metadata.list_multipart_uploads() {
            Ok(u) => u,
            Err(e) => {
                tracing::warn!(error = %e, "Failed to list multipart uploads for cleanup");
                continue;
            }
        };

        let now = chrono::Utc::now();
        let ttl_duration = chrono::Duration::seconds(ttl as i64);

        for upload in uploads {
            if upload.created + ttl_duration < now {
                tracing::info!(
                    upload_id = %upload.upload_id,
                    bucket = %upload.bucket,
                    key = %upload.key,
                    age_secs = now.signed_duration_since(upload.created).num_seconds(),
                    "Cleaning up expired multipart upload"
                );
                let _ = state.filestore.cleanup_multipart(&upload.upload_id).await;
                let _ = state.metadata.delete_multipart_upload(&upload.upload_id);
                metrics::counter!(simples3_server::metrics::MULTIPART_EXPIRED_TOTAL).increment(1);
            }
        }
    }
}

async fn lifecycle_expiration_loop(state: Arc<AppState>) {
    let interval_secs = state.config.lifecycle_scan_interval_secs;
    if interval_secs == 0 {
        tracing::info!("Lifecycle expiration scanner is disabled (interval = 0)");
        return;
    }

    tracing::info!(
        interval_secs = interval_secs,
        "Starting lifecycle expiration scanner"
    );

    let mut interval = tokio::time::interval(std::time::Duration::from_secs(interval_secs));
    // Skip first tick so we don't scan immediately on startup
    interval.tick().await;

    loop {
        interval.tick().await;

        let configs = match state.metadata.list_lifecycle_configurations() {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(error = %e, "Failed to list lifecycle configurations");
                continue;
            }
        };

        let now = chrono::Utc::now();

        for (bucket, config) in configs {
            for rule in &config.rules {
                if rule.status != simples3_core::s3::types::LifecycleStatus::Enabled {
                    continue;
                }

                let list_req = simples3_core::s3::types::ListObjectsV2Request {
                    bucket: bucket.clone(),
                    prefix: rule.prefix.clone(),
                    delimiter: String::new(),
                    max_keys: u32::MAX,
                    continuation_token: None,
                    start_after: None,
                };

                let objects = match state.metadata.list_objects_v2(&list_req) {
                    Ok(resp) => resp.contents,
                    Err(e) => {
                        tracing::warn!(bucket = %bucket, error = %e, "Failed to list objects for lifecycle");
                        continue;
                    }
                };

                for obj in objects {
                    // Tag matching: if rule has tags, all must match
                    if !rule.tags.is_empty() {
                        let obj_tags = state
                            .metadata
                            .get_object_tagging(&bucket, &obj.key)
                            .unwrap_or_default();
                        let all_match = rule.tags.iter().all(|rt| {
                            obj_tags.get(&rt.key).map_or(false, |v| v == &rt.value)
                        });
                        if !all_match {
                            continue;
                        }
                    }

                    // Determine if object should be expired
                    let should_expire = if let Some(ref date_str) = rule.expiration_date {
                        // Date-based expiration: expire if now >= date
                        if let Ok(exp_date) = chrono::DateTime::parse_from_rfc3339(date_str) {
                            now >= exp_date
                        } else {
                            false
                        }
                    } else {
                        // Days-based expiration
                        let expiration = chrono::Duration::days(rule.expiration_days as i64);
                        obj.last_modified + expiration < now
                    };

                    if should_expire {
                        tracing::info!(
                            bucket = %bucket,
                            key = %obj.key,
                            rule_id = %rule.id,
                            "Deleting expired object (lifecycle)"
                        );
                        let _ = state.metadata.delete_object_meta(&bucket, &obj.key);
                        let _ = state.filestore.delete_object(&bucket, &obj.key).await;
                        metrics::counter!(simples3_server::metrics::LIFECYCLE_EXPIRED_TOTAL).increment(1);
                    }
                }
            }
        }
    }
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to install CTRL+C handler");
    tracing::info!("Shutdown signal received");
}
