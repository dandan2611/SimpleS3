mod common;

use common::TestServer;
use serde_json::Value;

const ADMIN_TOKEN: &str = "test-admin-token";

fn admin_client() -> reqwest::Client {
    reqwest::Client::new()
}

#[tokio::test]
async fn test_admin_create_and_list_buckets() {
    let server = TestServer::start_with_admin_token(ADMIN_TOKEN).await;
    let client = admin_client();

    // Create bucket via admin API
    let resp = client
        .put(format!("{}/_admin/buckets/admin-bucket", server.admin_base_url))
        .header("Authorization", format!("Bearer {}", ADMIN_TOKEN))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);

    // List buckets via admin API
    let resp = client
        .get(format!("{}/_admin/buckets", server.admin_base_url))
        .header("Authorization", format!("Bearer {}", ADMIN_TOKEN))
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
    let server = TestServer::start_with_admin_token(ADMIN_TOKEN).await;
    let client = admin_client();

    client
        .put(format!("{}/_admin/buckets/del-me", server.admin_base_url))
        .header("Authorization", format!("Bearer {}", ADMIN_TOKEN))
        .send()
        .await
        .unwrap();

    let resp = client
        .delete(format!("{}/_admin/buckets/del-me", server.admin_base_url))
        .header("Authorization", format!("Bearer {}", ADMIN_TOKEN))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);

    let resp = client
        .get(format!("{}/_admin/buckets", server.admin_base_url))
        .header("Authorization", format!("Bearer {}", ADMIN_TOKEN))
        .send()
        .await
        .unwrap();
    let buckets: Vec<Value> = resp.json().await.unwrap();
    assert!(buckets.is_empty());
}

#[tokio::test]
async fn test_admin_set_anonymous() {
    let server = TestServer::start_with_admin_token(ADMIN_TOKEN).await;
    let client = admin_client();

    client
        .put(format!("{}/_admin/buckets/anon-test", server.admin_base_url))
        .header("Authorization", format!("Bearer {}", ADMIN_TOKEN))
        .send()
        .await
        .unwrap();

    let resp = client
        .put(format!(
            "{}/_admin/buckets/anon-test/anonymous",
            server.admin_base_url
        ))
        .header("Authorization", format!("Bearer {}", ADMIN_TOKEN))
        .json(&serde_json::json!({ "enabled": true }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let resp = client
        .get(format!("{}/_admin/buckets", server.admin_base_url))
        .header("Authorization", format!("Bearer {}", ADMIN_TOKEN))
        .send()
        .await
        .unwrap();
    let buckets: Vec<Value> = resp.json().await.unwrap();
    assert_eq!(buckets[0]["anonymous_read"], true);
}

#[tokio::test]
async fn test_admin_create_and_list_credentials() {
    let server = TestServer::start_with_admin_token(ADMIN_TOKEN).await;
    let client = admin_client();

    // Create credential via admin API
    let resp = client
        .post(format!("{}/_admin/credentials", server.admin_base_url))
        .header("Authorization", format!("Bearer {}", ADMIN_TOKEN))
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
        .get(format!("{}/_admin/credentials", server.admin_base_url))
        .header("Authorization", format!("Bearer {}", ADMIN_TOKEN))
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
    let server = TestServer::start_with_admin_token(ADMIN_TOKEN).await;
    let client = admin_client();

    // Create credential
    let resp = client
        .post(format!("{}/_admin/credentials", server.admin_base_url))
        .header("Authorization", format!("Bearer {}", ADMIN_TOKEN))
        .json(&serde_json::json!({ "description": "to revoke" }))
        .send()
        .await
        .unwrap();
    let cred: Value = resp.json().await.unwrap();
    let akid = cred["access_key_id"].as_str().unwrap();

    // Revoke it
    let resp = client
        .delete(format!("{}/_admin/credentials/{}", server.admin_base_url, akid))
        .header("Authorization", format!("Bearer {}", ADMIN_TOKEN))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Verify it's inactive
    let resp = client
        .get(format!("{}/_admin/credentials", server.admin_base_url))
        .header("Authorization", format!("Bearer {}", ADMIN_TOKEN))
        .send()
        .await
        .unwrap();
    let creds: Vec<Value> = resp.json().await.unwrap();
    let revoked = creds.iter().find(|c| c["access_key_id"] == akid).unwrap();
    assert_eq!(revoked["active"], false);
}

#[tokio::test]
async fn test_admin_api_not_on_s3_port() {
    // Admin routes should NOT be served on the S3 port
    let server = TestServer::start_with_admin_token(ADMIN_TOKEN).await;
    let client = admin_client();

    // Admin on admin port works with token
    let resp = client
        .get(format!("{}/_admin/buckets", server.admin_base_url))
        .header("Authorization", format!("Bearer {}", ADMIN_TOKEN))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Admin on S3 port should fail (S3 auth error or not found)
    let resp = client
        .get(format!("{}/_admin/buckets", server.base_url))
        .send()
        .await
        .unwrap();
    // S3 port doesn't have admin routes, so this hits the S3 fallback dispatcher
    // which will return a 403 (auth required) or some S3 error — not a 200
    assert_ne!(resp.status(), 200);

    // S3 API should still require auth on S3 port
    let resp = client.get(&server.base_url).send().await.unwrap();
    assert_eq!(resp.status(), 403);
}

#[tokio::test]
async fn test_admin_token_required_when_configured() {
    let server = TestServer::start_with_admin_token("supersecret").await;
    let client = admin_client();

    // Without token → 401
    let resp = client
        .get(format!("{}/_admin/buckets", server.admin_base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);

    // With wrong token → 401
    let resp = client
        .get(format!("{}/_admin/buckets", server.admin_base_url))
        .header("Authorization", "Bearer wrongtoken")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);

    // With correct token → 200
    let resp = client
        .get(format!("{}/_admin/buckets", server.admin_base_url))
        .header("Authorization", "Bearer supersecret")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_admin_no_token_when_unconfigured() {
    let server = TestServer::start().await;
    let client = admin_client();

    // No token configured → admin should be denied (401)
    let resp = client
        .get(format!("{}/_admin/buckets", server.admin_base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
}
