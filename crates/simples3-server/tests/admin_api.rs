mod common;

use common::TestServer;
use serde_json::Value;

#[tokio::test]
async fn test_admin_create_and_list_buckets() {
    let server = TestServer::start().await;
    let client = reqwest::Client::new();

    // Create bucket via admin API
    let resp = client
        .put(format!("{}/_admin/buckets/admin-bucket", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);

    // List buckets via admin API
    let resp = client
        .get(format!("{}/_admin/buckets", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let buckets: Vec<Value> = resp.json().await.unwrap();
    assert_eq!(buckets.len(), 1);
    assert_eq!(buckets[0]["name"], "admin-bucket");
}

#[tokio::test]
async fn test_admin_delete_bucket() {
    let server = TestServer::start().await;
    let client = reqwest::Client::new();

    client
        .put(format!("{}/_admin/buckets/del-me", server.base_url))
        .send()
        .await
        .unwrap();

    let resp = client
        .delete(format!("{}/_admin/buckets/del-me", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);

    let resp = client
        .get(format!("{}/_admin/buckets", server.base_url))
        .send()
        .await
        .unwrap();
    let buckets: Vec<Value> = resp.json().await.unwrap();
    assert!(buckets.is_empty());
}

#[tokio::test]
async fn test_admin_set_anonymous() {
    let server = TestServer::start().await;
    let client = reqwest::Client::new();

    client
        .put(format!("{}/_admin/buckets/anon-test", server.base_url))
        .send()
        .await
        .unwrap();

    let resp = client
        .put(format!(
            "{}/_admin/buckets/anon-test/anonymous",
            server.base_url
        ))
        .json(&serde_json::json!({ "enabled": true }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let resp = client
        .get(format!("{}/_admin/buckets", server.base_url))
        .send()
        .await
        .unwrap();
    let buckets: Vec<Value> = resp.json().await.unwrap();
    assert_eq!(buckets[0]["anonymous_read"], true);
}

#[tokio::test]
async fn test_admin_create_and_list_credentials() {
    let server = TestServer::start().await;
    let client = reqwest::Client::new();

    // Create credential via admin API
    let resp = client
        .post(format!("{}/_admin/credentials", server.base_url))
        .json(&serde_json::json!({ "description": "test key" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let cred: Value = resp.json().await.unwrap();
    assert!(cred["access_key_id"].as_str().unwrap().starts_with("AKID"));
    assert!(!cred["secret_access_key"].as_str().unwrap().is_empty());

    // List credentials (should have 2: the test fixture one + the new one)
    let resp = client
        .get(format!("{}/_admin/credentials", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let creds: Vec<Value> = resp.json().await.unwrap();
    assert_eq!(creds.len(), 2);
    // Secrets should be masked in list
    assert_eq!(creds[0]["secret_access_key"], "********");
}

#[tokio::test]
async fn test_admin_revoke_credential() {
    let server = TestServer::start().await;
    let client = reqwest::Client::new();

    // Create credential
    let resp = client
        .post(format!("{}/_admin/credentials", server.base_url))
        .json(&serde_json::json!({ "description": "to revoke" }))
        .send()
        .await
        .unwrap();
    let cred: Value = resp.json().await.unwrap();
    let akid = cred["access_key_id"].as_str().unwrap();

    // Revoke it
    let resp = client
        .delete(format!("{}/_admin/credentials/{}", server.base_url, akid))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Verify it's inactive
    let resp = client
        .get(format!("{}/_admin/credentials", server.base_url))
        .send()
        .await
        .unwrap();
    let creds: Vec<Value> = resp.json().await.unwrap();
    let revoked = creds.iter().find(|c| c["access_key_id"] == akid).unwrap();
    assert_eq!(revoked["active"], false);
}

#[tokio::test]
async fn test_admin_api_bypasses_s3_auth() {
    // Admin API should work even without anonymous_global or SigV4 credentials
    let server = TestServer::start().await;
    let client = reqwest::Client::new();

    // This should work (admin API has no auth) even though the server
    // is NOT in anonymous_global mode
    let resp = client
        .get(format!("{}/_admin/buckets", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // But S3 API should still require auth
    let resp = client.get(&server.base_url).send().await.unwrap();
    assert_eq!(resp.status(), 403);
}
