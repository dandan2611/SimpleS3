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
            public: false,
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

#[tokio::test]
async fn test_anonymous_get_public_object_on_private_bucket() {
    let server = TestServer::start().await;
    let client = reqwest::Client::new();

    // Create a private bucket (no anonymous_read)
    server.metadata.create_bucket("private-bucket").unwrap();

    // Store a public object directly via metadata
    server
        .metadata
        .put_object_meta(&simples3_core::s3::types::ObjectMeta {
            bucket: "private-bucket".into(),
            key: "public-file.txt".into(),
            size: 5,
            etag: "abc".into(),
            content_type: "text/plain".into(),
            last_modified: chrono::Utc::now(),
            public: true,
        })
        .unwrap();

    // Store a private object
    server
        .metadata
        .put_object_meta(&simples3_core::s3::types::ObjectMeta {
            bucket: "private-bucket".into(),
            key: "private-file.txt".into(),
            size: 5,
            etag: "def".into(),
            content_type: "text/plain".into(),
            last_modified: chrono::Utc::now(),
            public: false,
        })
        .unwrap();

    // Anonymous HEAD on public object should succeed
    let resp = client
        .head(format!(
            "{}/private-bucket/public-file.txt",
            server.base_url
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Anonymous HEAD on private object should be denied
    let resp = client
        .head(format!(
            "{}/private-bucket/private-file.txt",
            server.base_url
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);
}

#[tokio::test]
async fn test_anonymous_list_public_objects_only() {
    let server = TestServer::start().await;
    let client = reqwest::Client::new();

    // Create bucket with anonymous_list_public enabled
    server.metadata.create_bucket("list-pub").unwrap();
    server
        .metadata
        .set_bucket_anonymous_list_public("list-pub", true)
        .unwrap();

    // Store public and private objects
    server
        .metadata
        .put_object_meta(&simples3_core::s3::types::ObjectMeta {
            bucket: "list-pub".into(),
            key: "public.txt".into(),
            size: 5,
            etag: "a".into(),
            content_type: "text/plain".into(),
            last_modified: chrono::Utc::now(),
            public: true,
        })
        .unwrap();
    server
        .metadata
        .put_object_meta(&simples3_core::s3::types::ObjectMeta {
            bucket: "list-pub".into(),
            key: "secret.txt".into(),
            size: 5,
            etag: "b".into(),
            content_type: "text/plain".into(),
            last_modified: chrono::Utc::now(),
            public: false,
        })
        .unwrap();

    // Anonymous list should only show public objects
    let resp = client
        .get(format!("{}/list-pub?list-type=2", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(body.contains("<Key>public.txt</Key>"));
    assert!(!body.contains("<Key>secret.txt</Key>"));
    assert!(body.contains("<KeyCount>1</KeyCount>"));
}

#[tokio::test]
async fn test_anonymous_list_denied_without_flag() {
    let server = TestServer::start().await;
    let client = reqwest::Client::new();

    // Create bucket without anonymous_list_public
    server.metadata.create_bucket("no-list").unwrap();

    // Anonymous list should be denied
    let resp = client
        .get(format!("{}/no-list?list-type=2", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);
}
