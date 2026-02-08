mod common;

use common::TestServer;

#[tokio::test]
async fn test_create_and_list_buckets() {
    let server = TestServer::start_anonymous().await;
    let client = reqwest::Client::new();

    // Create bucket
    let resp = client
        .put(format!("{}/test-bucket", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // List buckets
    let resp = client.get(&server.base_url).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(body.contains("<Name>test-bucket</Name>"));
}

#[tokio::test]
async fn test_delete_bucket() {
    let server = TestServer::start_anonymous().await;
    let client = reqwest::Client::new();

    client
        .put(format!("{}/del-bucket", server.base_url))
        .send()
        .await
        .unwrap();

    let resp = client
        .delete(format!("{}/del-bucket", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);

    // Verify gone from list
    let resp = client.get(&server.base_url).send().await.unwrap();
    let body = resp.text().await.unwrap();
    assert!(!body.contains("<Name>del-bucket</Name>"));
}

#[tokio::test]
async fn test_head_bucket() {
    let server = TestServer::start_anonymous().await;
    let client = reqwest::Client::new();

    // 404 for nonexistent
    let resp = client
        .head(format!("{}/nonexistent", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);

    // Create and head
    client
        .put(format!("{}/head-bucket", server.base_url))
        .send()
        .await
        .unwrap();

    let resp = client
        .head(format!("{}/head-bucket", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_delete_nonempty_bucket_returns_409() {
    let server = TestServer::start_anonymous().await;
    let client = reqwest::Client::new();

    client
        .put(format!("{}/nonempty", server.base_url))
        .send()
        .await
        .unwrap();

    // Put an object
    client
        .put(format!("{}/nonempty/file.txt", server.base_url))
        .body("hello")
        .send()
        .await
        .unwrap();

    // Try delete bucket
    let resp = client
        .delete(format!("{}/nonempty", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 409);
}
