mod common;

use common::TestServer;

#[tokio::test]
async fn test_health_endpoint() {
    let server = TestServer::start().await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("{}/health", server.admin_base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.text().await.unwrap(), "ok");
}

#[tokio::test]
async fn test_ready_endpoint() {
    let server = TestServer::start().await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("{}/ready", server.admin_base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.text().await.unwrap(), "ready");
}

#[tokio::test]
async fn test_metrics_endpoint() {
    let server = TestServer::start().await;
    let client = reqwest::Client::new();

    // Create a bucket so we have some data
    server.metadata.create_bucket("metrics-test").unwrap();

    let resp = client
        .get(format!("{}/metrics", server.admin_base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let body = resp.text().await.unwrap();
    assert!(body.contains("simples3_bucket_count"));
    assert!(body.contains("simples3_uptime_seconds"));
    assert!(body.contains("simples3_total_object_count"));
    assert!(body.contains("simples3_total_storage_bytes"));
    assert!(body.contains("simples3_credential_count"));
    assert!(body.contains("simples3_active_multipart_uploads"));
}

#[tokio::test]
async fn test_health_endpoints_no_auth_required() {
    let server = TestServer::start_with_admin_token("supersecret").await;
    let client = reqwest::Client::new();

    // Health, ready, metrics should work without auth
    let resp = client
        .get(format!("{}/health", server.admin_base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let resp = client
        .get(format!("{}/ready", server.admin_base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let resp = client
        .get(format!("{}/metrics", server.admin_base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Admin endpoint should require auth
    let resp = client
        .get(format!("{}/_admin/buckets", server.admin_base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn test_metrics_request_counters() {
    let server = TestServer::start().await;
    let client = reqwest::Client::new();

    // Make some S3 requests to generate metrics
    client
        .put(format!("{}/counter-bucket", server.base_url))
        .send()
        .await
        .unwrap();

    // Give a moment for the request to be processed
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let resp = client
        .get(format!("{}/metrics", server.admin_base_url))
        .send()
        .await
        .unwrap();
    let body = resp.text().await.unwrap();

    assert!(body.contains("s3_requests_total"));
    assert!(body.contains("s3_request_duration_seconds"));
}
