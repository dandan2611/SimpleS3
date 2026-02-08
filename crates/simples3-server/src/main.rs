use clap::Parser;
use simples3_core::Config;
use simples3_server::{AppState, router};
use std::path::Path;
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

    let state = Arc::new(AppState {
        config: config.clone(),
        metadata,
        filestore,
    });

    let s3_app = router::build_s3_router(state.clone());
    let s3_listener = tokio::net::TcpListener::bind(&config.bind)
        .await
        .expect("Failed to bind S3 listener");
    tracing::info!("simples3 S3 API listening on {}", config.bind);

    if config.admin_enabled {
        let admin_app = router::build_admin_router(state);
        let admin_listener = tokio::net::TcpListener::bind(&config.admin_bind)
            .await
            .expect("Failed to bind admin listener");
        tracing::info!("simples3 admin API listening on {}", config.admin_bind);

        let s3_handle = tokio::spawn(async move {
            axum::serve(s3_listener, s3_app)
                .with_graceful_shutdown(shutdown_signal())
                .await
                .expect("S3 server error");
        });

        let admin_handle = tokio::spawn(async move {
            axum::serve(admin_listener, admin_app).await.expect("Admin server error");
        });

        // Wait for S3 server to finish (shutdown signal), then drop admin
        let _ = s3_handle.await;
        admin_handle.abort();
    } else {
        tracing::info!("Admin API is disabled");
        axum::serve(s3_listener, s3_app)
            .with_graceful_shutdown(shutdown_signal())
            .await
            .expect("S3 server error");
    }
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to install CTRL+C handler");
    tracing::info!("Shutdown signal received");
}
