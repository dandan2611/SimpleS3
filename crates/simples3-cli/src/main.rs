use clap::{Parser, Subcommand};
use simples3_core::Config;

mod commands;

#[derive(Parser)]
#[command(name = "simples3-cli", about = "simples3 admin CLI")]
struct Cli {
    /// Server URL to connect to (default: http://localhost:9000)
    #[arg(long, default_value = "http://localhost:9000")]
    server_url: String,

    /// Operate directly on the sled database instead of via HTTP.
    /// Only works when the server is NOT running (sled uses exclusive locks).
    #[arg(long)]
    offline: bool,

    /// Metadata directory for offline mode (overrides SIMPLES3_METADATA_DIR)
    #[arg(long)]
    metadata_dir: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Bucket management
    Bucket {
        #[command(subcommand)]
        action: BucketAction,
    },
    /// Credential management
    Credentials {
        #[command(subcommand)]
        action: CredentialAction,
    },
}

#[derive(Subcommand)]
enum BucketAction {
    /// Create a new bucket
    Create { name: String },
    /// List all buckets
    List,
    /// Delete a bucket
    Delete { name: String },
    /// Configure bucket settings
    Config {
        name: String,
        #[command(subcommand)]
        setting: BucketConfigSetting,
    },
}

#[derive(Subcommand)]
enum BucketConfigSetting {
    /// Set anonymous read access (true or false)
    Anonymous {
        #[arg(value_parser = clap::value_parser!(bool))]
        value: bool,
    },
}

#[derive(Subcommand)]
enum CredentialAction {
    /// Create a new access key
    Create {
        #[arg(long, default_value = "")]
        description: String,
    },
    /// List all credentials
    List,
    /// Revoke an access key
    Revoke { access_key_id: String },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    if cli.offline {
        run_offline(cli);
    } else {
        run_online(cli).await;
    }
}

fn run_offline(cli: Cli) {
    let mut config = Config::from_env();
    if let Some(metadata_dir) = cli.metadata_dir {
        config.metadata_dir = metadata_dir.into();
    }

    std::fs::create_dir_all(&config.metadata_dir).expect("Failed to create metadata directory");

    let store = simples3_core::storage::MetadataStore::open(&config.metadata_dir)
        .expect("Failed to open metadata store");

    match cli.command {
        Commands::Bucket { action } => match action {
            BucketAction::Create { name } => commands::bucket::create_offline(&store, &name),
            BucketAction::List => commands::bucket::list_offline(&store),
            BucketAction::Delete { name } => commands::bucket::delete_offline(&store, &name),
            BucketAction::Config { name, setting } => match setting {
                BucketConfigSetting::Anonymous { value } => {
                    commands::bucket::set_anonymous_offline(&store, &name, value)
                }
            },
        },
        Commands::Credentials { action } => match action {
            CredentialAction::Create { description } => {
                commands::credentials::create_offline(&store, &description)
            }
            CredentialAction::List => commands::credentials::list_offline(&store),
            CredentialAction::Revoke { access_key_id } => {
                commands::credentials::revoke_offline(&store, &access_key_id)
            }
        },
    }
}

async fn run_online(cli: Cli) {
    let base = cli.server_url.trim_end_matches('/').to_string();
    let client = reqwest::Client::new();

    match cli.command {
        Commands::Bucket { action } => match action {
            BucketAction::Create { name } => {
                commands::bucket::create_online(&client, &base, &name).await
            }
            BucketAction::List => commands::bucket::list_online(&client, &base).await,
            BucketAction::Delete { name } => {
                commands::bucket::delete_online(&client, &base, &name).await
            }
            BucketAction::Config { name, setting } => match setting {
                BucketConfigSetting::Anonymous { value } => {
                    commands::bucket::set_anonymous_online(&client, &base, &name, value).await
                }
            },
        },
        Commands::Credentials { action } => match action {
            CredentialAction::Create { description } => {
                commands::credentials::create_online(&client, &base, &description).await
            }
            CredentialAction::List => commands::credentials::list_online(&client, &base).await,
            CredentialAction::Revoke { access_key_id } => {
                commands::credentials::revoke_online(&client, &base, &access_key_id).await
            }
        },
    }
}
