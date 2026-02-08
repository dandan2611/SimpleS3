use simples3_core::Config;
use simples3_core::storage::{FileStore, MetadataStore};
use std::net::SocketAddr;
use std::sync::Arc;

pub struct TestServer {
    pub addr: SocketAddr,
    pub base_url: String,
    pub admin_addr: SocketAddr,
    pub admin_base_url: String,
    pub metadata: MetadataStore,
    _data_dir: tempfile::TempDir,
    _metadata_dir: tempfile::TempDir,
}

impl TestServer {
    pub async fn start() -> Self {
        Self::start_inner(false, None).await
    }

    pub async fn start_anonymous() -> Self {
        Self::start_inner(true, None).await
    }

    pub async fn start_with_admin_token(token: &str) -> Self {
        Self::start_inner(false, Some(token.to_string())).await
    }

    async fn start_inner(anonymous_global: bool, admin_token: Option<String>) -> Self {
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
            admin_enabled: true,
            admin_bind: "127.0.0.1:0".into(),
            admin_token,
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

        let s3_app = simples3_server::router::build_s3_router(state.clone());
        let s3_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = s3_listener.local_addr().unwrap();

        let admin_app = simples3_server::router::build_admin_router(state);
        let admin_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let admin_addr = admin_listener.local_addr().unwrap();

        tokio::spawn(async move {
            axum::serve(s3_listener, s3_app).await.unwrap();
        });

        tokio::spawn(async move {
            axum::serve(admin_listener, admin_app).await.unwrap();
        });

        Self {
            base_url: format!("http://{}", addr),
            addr,
            admin_base_url: format!("http://{}", admin_addr),
            admin_addr,
            metadata,
            _data_dir: data_dir,
            _metadata_dir: metadata_dir,
        }
    }
}
