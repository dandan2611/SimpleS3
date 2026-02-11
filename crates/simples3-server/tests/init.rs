mod common;

use std::io::Write;

#[tokio::test]
async fn test_server_init_config() {
    let mut tmpfile = tempfile::NamedTempFile::new().unwrap();
    write!(
        tmpfile,
        r#"
[[buckets]]
name = "init-bucket-1"

[[buckets]]
name = "init-bucket-2"
anonymous_read = true

[[credentials]]
access_key_id = "AKID_INIT"
secret_access_key = "secret_init_123"
description = "init credential"
"#
    )
    .unwrap();
    tmpfile.flush().unwrap();

    let server = common::TestServer::start_with_init_config(tmpfile.path()).await;

    let client = reqwest::Client::new();

    // Verify buckets were created via admin API
    let resp = client
        .get(format!("{}/_admin/buckets", server.admin_base_url))
        .header("Authorization", "Bearer init-admin-token")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    let buckets = body.as_array().unwrap();

    let bucket_names: Vec<&str> = buckets
        .iter()
        .map(|b| b["name"].as_str().unwrap())
        .collect();
    assert!(bucket_names.contains(&"init-bucket-1"));
    assert!(bucket_names.contains(&"init-bucket-2"));

    // Verify anonymous_read on init-bucket-2
    let bucket2 = buckets
        .iter()
        .find(|b| b["name"] == "init-bucket-2")
        .unwrap();
    assert_eq!(bucket2["anonymous_read"], true);

    // Verify init-bucket-1 does NOT have anonymous_read
    let bucket1 = buckets
        .iter()
        .find(|b| b["name"] == "init-bucket-1")
        .unwrap();
    assert_eq!(bucket1["anonymous_read"], false);

    // Verify credentials were created via admin API
    let resp = client
        .get(format!("{}/_admin/credentials", server.admin_base_url))
        .header("Authorization", "Bearer init-admin-token")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    let creds = body.as_array().unwrap();

    let cred_ids: Vec<&str> = creds
        .iter()
        .map(|c| c["access_key_id"].as_str().unwrap())
        .collect();
    assert!(cred_ids.contains(&"AKID_INIT"));
    // TESTAKID is also created by TestServer
    assert!(cred_ids.contains(&"TESTAKID"));
}

#[tokio::test]
async fn test_server_init_config_idempotent() {
    let mut tmpfile = tempfile::NamedTempFile::new().unwrap();
    write!(
        tmpfile,
        r#"
[[buckets]]
name = "idem-test"

[[credentials]]
access_key_id = "AKID_IDEM_TEST"
secret_access_key = "secret"
description = "idem"
"#
    )
    .unwrap();
    tmpfile.flush().unwrap();

    // First server applies the init config
    let server = common::TestServer::start_with_init_config(tmpfile.path()).await;

    // Apply again on the same metadata store — should not error
    let init_cfg = simples3_core::init::load(tmpfile.path()).unwrap();
    simples3_core::init::apply(&init_cfg, &server.metadata).unwrap();

    // Still only one bucket with that name
    let buckets = server.metadata.list_buckets().unwrap();
    let count = buckets.iter().filter(|b| b.name == "idem-test").count();
    assert_eq!(count, 1);
}
