mod common;

use common::TestServer;

#[tokio::test]
async fn test_unauthenticated_request_denied() {
    let server = TestServer::start().await;
    let client = reqwest::Client::new();

    // Without any auth header, should get 403
    let resp = client.get(&server.base_url).send().await.unwrap();
    assert_eq!(resp.status(), 403);
}

#[tokio::test]
async fn test_anonymous_read_on_enabled_bucket() {
    let server = TestServer::start().await;
    let client = reqwest::Client::new();

    // Create bucket and enable anonymous read (via metadata store directly)
    server.metadata.create_bucket("public-bucket").unwrap();
    server
        .metadata
        .set_bucket_anonymous_read("public-bucket", true)
        .unwrap();

    // PUT object requires auth, so we store metadata+file directly for this test
    server
        .metadata
        .put_object_meta(&simples3_core::s3::types::ObjectMeta {
            bucket: "public-bucket".into(),
            key: "public-file.txt".into(),
            size: 5,
            etag: "abc".into(),
            content_type: "text/plain".into(),
            last_modified: chrono::Utc::now(),
        })
        .unwrap();

    // Anonymous GET on enabled bucket should succeed (HeadBucket at least)
    let resp = client
        .head(format!("{}/public-bucket", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_anonymous_write_denied() {
    let server = TestServer::start().await;
    let client = reqwest::Client::new();

    // Create bucket with anonymous read
    server.metadata.create_bucket("anon-write").unwrap();
    server
        .metadata
        .set_bucket_anonymous_read("anon-write", true)
        .unwrap();

    // Anonymous PUT should be denied (write is not read-only)
    let resp = client
        .put(format!("{}/anon-write/file.txt", server.base_url))
        .body("data")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);
}
