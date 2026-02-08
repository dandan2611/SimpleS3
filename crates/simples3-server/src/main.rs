use clap::Parser;
use simples3_core::Config;
use simples3_server::{AppState, router};
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

    let state = Arc::new(AppState {
        config: config.clone(),
        metadata,
        filestore,
    });

    let app = router::build_router(state);

    let listener = tokio::net::TcpListener::bind(&config.bind)
        .await
        .expect("Failed to bind");
    tracing::info!("simples3 listening on {}", config.bind);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("Server error");
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to install CTRL+C handler");
    tracing::info!("Shutdown signal received");
}
