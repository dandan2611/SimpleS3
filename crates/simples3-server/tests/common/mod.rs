use simples3_core::Config;
use simples3_core::storage::{FileStore, MetadataStore};
use std::net::SocketAddr;
use std::sync::Arc;

pub struct TestServer {
    pub addr: SocketAddr,
    pub base_url: String,
    pub metadata: MetadataStore,
    _data_dir: tempfile::TempDir,
    _metadata_dir: tempfile::TempDir,
}

impl TestServer {
    pub async fn start() -> Self {
        Self::start_with_anonymous(false).await
    }

    pub async fn start_anonymous() -> Self {
        Self::start_with_anonymous(true).await
    }

    async fn start_with_anonymous(anonymous_global: bool) -> Self {
        let data_dir = tempfile::tempdir().unwrap();
        let metadata_dir = tempfile::tempdir().unwrap();

        let config = Config {
            bind: "127.0.0.1:0".into(),
            data_dir: data_dir.path().to_path_buf(),
            metadata_dir: metadata_dir.path().to_path_buf(),
            hostname: "s3.localhost".into(),
            region: "us-east-1".into(),
            log_level: "warn".into(),
            anonymous_global,
        };

        let metadata = MetadataStore::open(&config.metadata_dir).unwrap();
        let filestore = FileStore::new(&config.data_dir);

        metadata
            .create_credential("TESTAKID", "TESTSECRET", "test")
            .unwrap();

        let state = Arc::new(simples3_server::AppState {
            config,
            metadata: metadata.clone(),
            filestore,
        });

        let app = simples3_server::router::build_router(state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        Self {
            base_url: format!("http://{}", addr),
            addr,
            metadata,
            _data_dir: data_dir,
            _metadata_dir: metadata_dir,
        }
    }
}
