use serde::Deserialize;
use simples3_core::auth::credentials;
use simples3_core::storage::MetadataStore;
use tabled::{Table, Tabled};

#[derive(Tabled, Deserialize)]
struct CredentialRow {
    #[tabled(rename = "Access Key ID")]
    access_key_id: String,
    #[tabled(rename = "Description")]
    description: String,
    #[tabled(rename = "Created")]
    created: String,
    #[tabled(rename = "Active")]
    active: bool,
}

#[derive(Deserialize)]
struct CreatedCredential {
    access_key_id: String,
    secret_access_key: String,
}

// --- Offline (direct sled) ---

pub fn create_offline(store: &MetadataStore, description: &str) {
    let access_key_id = credentials::generate_access_key_id();
    let secret_access_key = credentials::generate_secret_access_key();

    match store.create_credential(&access_key_id, &secret_access_key, description) {
        Ok(record) => {
            println!("Credential created:");
            println!("  Access Key ID:     {}", record.access_key_id);
            println!("  Secret Access Key: {}", record.secret_access_key);
            println!();
            println!("Save the secret access key — it cannot be retrieved later.");
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

pub fn list_offline(store: &MetadataStore) {
    match store.list_credentials() {
        Ok(creds) => {
            if creds.is_empty() {
                println!("No credentials found.");
                return;
            }
            let rows: Vec<CredentialRow> = creds
                .into_iter()
                .map(|c| CredentialRow {
                    access_key_id: c.access_key_id,
                    description: c.description,
                    created: c.created.to_rfc3339(),
                    active: c.active,
                })
                .collect();
            println!("{}", Table::new(rows));
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

pub fn revoke_offline(store: &MetadataStore, access_key_id: &str) {
    match store.revoke_credential(access_key_id) {
        Ok(()) => println!("Credential '{}' revoked.", access_key_id),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

// --- Online (HTTP to server) ---

pub async fn create_online(client: &reqwest::Client, base: &str, description: &str) {
    let resp = client
        .post(format!("{}/_admin/credentials", base))
        .json(&serde_json::json!({ "description": description }))
        .send()
        .await;
    match resp {
        Ok(r) if r.status().is_success() => {
            let cred: CreatedCredential = match r.json().await {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Error parsing response: {}", e);
                    std::process::exit(1);
                }
            };
            println!("Credential created:");
            println!("  Access Key ID:     {}", cred.access_key_id);
            println!("  Secret Access Key: {}", cred.secret_access_key);
            println!();
            println!("Save the secret access key — it cannot be retrieved later.");
        }
        Ok(r) => {
            eprintln!("Error: server returned {}", r.status());
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

pub async fn list_online(client: &reqwest::Client, base: &str) {
    let resp = client
        .get(format!("{}/_admin/credentials", base))
        .send()
        .await;
    match resp {
        Ok(r) if r.status().is_success() => {
            let creds: Vec<CredentialRow> = r.json().await.unwrap_or_default();
            if creds.is_empty() {
                println!("No credentials found.");
                return;
            }
            println!("{}", Table::new(creds));
        }
        Ok(r) => {
            eprintln!("Error: server returned {}", r.status());
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

pub async fn revoke_online(client: &reqwest::Client, base: &str, access_key_id: &str) {
    let resp = client
        .delete(format!("{}/_admin/credentials/{}", base, access_key_id))
        .send()
        .await;
    match resp {
        Ok(r) if r.status().is_success() => {
            println!("Credential '{}' revoked.", access_key_id)
        }
        Ok(r) => {
            eprintln!("Error: server returned {}", r.status());
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
