use serde::Deserialize;
use simples3_core::storage::MetadataStore;
use tabled::{Table, Tabled};

#[derive(Tabled, Deserialize)]
struct BucketRow {
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Created")]
    #[serde(rename = "creation_date")]
    created: String,
    #[tabled(rename = "Anonymous Read")]
    anonymous_read: bool,
}

// --- Offline (direct sled) ---

pub fn create_offline(store: &MetadataStore, name: &str) {
    match store.create_bucket(name) {
        Ok(meta) => println!("Bucket '{}' created at {}", meta.name, meta.creation_date),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

pub fn list_offline(store: &MetadataStore) {
    match store.list_buckets() {
        Ok(buckets) => {
            if buckets.is_empty() {
                println!("No buckets found.");
                return;
            }
            let rows: Vec<BucketRow> = buckets
                .into_iter()
                .map(|b| BucketRow {
                    name: b.name,
                    created: b.creation_date.to_rfc3339(),
                    anonymous_read: b.anonymous_read,
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

pub fn delete_offline(store: &MetadataStore, name: &str) {
    match store.delete_bucket(name) {
        Ok(()) => println!("Bucket '{}' deleted.", name),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

pub fn set_anonymous_offline(store: &MetadataStore, name: &str, enabled: bool) {
    match store.set_bucket_anonymous_read(name, enabled) {
        Ok(()) => println!(
            "Anonymous read on '{}' set to {}.",
            name,
            if enabled { "enabled" } else { "disabled" }
        ),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

// --- Online (HTTP to server) ---

pub async fn create_online(client: &reqwest::Client, base: &str, name: &str) {
    let resp = client
        .put(format!("{}/_admin/buckets/{}", base, name))
        .send()
        .await;
    match resp {
        Ok(r) if r.status().is_success() => println!("Bucket '{}' created.", name),
        Ok(r) => {
            eprintln!("Error: server returned {}", r.status());
            if let Ok(body) = r.text().await {
                if !body.is_empty() {
                    eprintln!("{}", body);
                }
            }
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
        .get(format!("{}/_admin/buckets", base))
        .send()
        .await;
    match resp {
        Ok(r) if r.status().is_success() => {
            let buckets: Vec<BucketRow> = r.json().await.unwrap_or_default();
            if buckets.is_empty() {
                println!("No buckets found.");
                return;
            }
            println!("{}", Table::new(buckets));
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

pub async fn delete_online(client: &reqwest::Client, base: &str, name: &str) {
    let resp = client
        .delete(format!("{}/_admin/buckets/{}", base, name))
        .send()
        .await;
    match resp {
        Ok(r) if r.status().is_success() => println!("Bucket '{}' deleted.", name),
        Ok(r) => {
            eprintln!("Error: server returned {}", r.status());
            if let Ok(body) = r.text().await {
                if !body.is_empty() {
                    eprintln!("{}", body);
                }
            }
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

pub async fn set_anonymous_online(
    client: &reqwest::Client,
    base: &str,
    name: &str,
    enabled: bool,
) {
    let resp = client
        .put(format!("{}/_admin/buckets/{}/anonymous", base, name))
        .json(&serde_json::json!({ "enabled": enabled }))
        .send()
        .await;
    match resp {
        Ok(r) if r.status().is_success() => println!(
            "Anonymous read on '{}' set to {}.",
            name,
            if enabled { "enabled" } else { "disabled" }
        ),
        Ok(r) => {
            eprintln!("Error: server returned {}", r.status());
            if let Ok(body) = r.text().await {
                if !body.is_empty() {
                    eprintln!("{}", body);
                }
            }
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
