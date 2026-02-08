pub mod handlers;
pub mod middleware;
pub mod router;

pub struct AppState {
    pub config: simples3_core::Config,
    pub metadata: simples3_core::storage::MetadataStore,
    pub filestore: simples3_core::storage::FileStore,
}
