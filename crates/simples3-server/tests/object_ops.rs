mod common;

use common::TestServer;

async fn create_bucket(client: &reqwest::Client, base_url: &str, name: &str) {
    client
        .put(format!("{}/{}", base_url, name))
        .send()
        .await
        .unwrap();
}

#[tokio::test]
async fn test_put_and_get_object() {
    let server = TestServer::start_anonymous().await;
    let client = reqwest::Client::new();
    create_bucket(&client, &server.base_url, "obj-bucket").await;

    let data = "hello, s3 world!";
    let resp = client
        .put(format!("{}/obj-bucket/hello.txt", server.base_url))
        .body(data)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert!(resp.headers().get("etag").is_some());

    let resp = client
        .get(format!("{}/obj-bucket/hello.txt", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert_eq!(body, data);
}

#[tokio::test]
async fn test_head_object() {
    let server = TestServer::start_anonymous().await;
    let client = reqwest::Client::new();
    create_bucket(&client, &server.base_url, "head-obj").await;

    client
        .put(format!("{}/head-obj/file.txt", server.base_url))
        .header("content-type", "text/plain")
        .body("content")
        .send()
        .await
        .unwrap();

    let resp = client
        .head(format!("{}/head-obj/file.txt", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert!(resp.headers().get("content-length").is_some());
    assert!(resp.headers().get("etag").is_some());
    assert!(resp.headers().get("last-modified").is_some());
}

#[tokio::test]
async fn test_delete_object() {
    let server = TestServer::start_anonymous().await;
    let client = reqwest::Client::new();
    create_bucket(&client, &server.base_url, "del-obj").await;

    client
        .put(format!("{}/del-obj/to-delete.txt", server.base_url))
        .body("data")
        .send()
        .await
        .unwrap();

    let resp = client
        .delete(format!("{}/del-obj/to-delete.txt", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);

    let resp = client
        .get(format!("{}/del-obj/to-delete.txt", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn test_get_nonexistent_returns_404() {
    let server = TestServer::start_anonymous().await;
    let client = reqwest::Client::new();
    create_bucket(&client, &server.base_url, "404-bucket").await;

    let resp = client
        .get(format!("{}/404-bucket/nope.txt", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
    let body = resp.text().await.unwrap();
    assert!(body.contains("<Code>NoSuchKey</Code>"));
}

#[tokio::test]
async fn test_list_objects_v2() {
    let server = TestServer::start_anonymous().await;
    let client = reqwest::Client::new();
    create_bucket(&client, &server.base_url, "list-bucket").await;

    for key in ["photos/a.jpg", "photos/b.jpg", "docs/c.pdf"] {
        client
            .put(format!("{}/list-bucket/{}", server.base_url, key))
            .body("data")
            .send()
            .await
            .unwrap();
    }

    let resp = client
        .get(format!(
            "{}/list-bucket?list-type=2&prefix=photos/",
            server.base_url
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(body.contains("<Key>photos/a.jpg</Key>"));
    assert!(body.contains("<Key>photos/b.jpg</Key>"));
    assert!(!body.contains("<Key>docs/c.pdf</Key>"));
}

#[tokio::test]
async fn test_put_object_preserves_content_type() {
    let server = TestServer::start_anonymous().await;
    let client = reqwest::Client::new();
    create_bucket(&client, &server.base_url, "ct-bucket").await;

    client
        .put(format!("{}/ct-bucket/image.png", server.base_url))
        .header("content-type", "image/png")
        .body(vec![0u8; 100])
        .send()
        .await
        .unwrap();

    let resp = client
        .get(format!("{}/ct-bucket/image.png", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.headers().get("content-type").unwrap().to_str().unwrap(),
        "image/png"
    );
}

#[tokio::test]
async fn test_large_object_streaming() {
    let server = TestServer::start_anonymous().await;
    let client = reqwest::Client::new();
    create_bucket(&client, &server.base_url, "large-bucket").await;

    let data = vec![42u8; 10 * 1024 * 1024]; // 10MB
    let resp = client
        .put(format!("{}/large-bucket/big.bin", server.base_url))
        .body(data.clone())
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let resp = client
        .get(format!("{}/large-bucket/big.bin", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body = resp.bytes().await.unwrap();
    assert_eq!(body.len(), 10 * 1024 * 1024);
    assert_eq!(&body[..], &data[..]);
}

// --- Tagging tests ---

#[tokio::test]
async fn test_object_tagging_lifecycle() {
    let server = TestServer::start_anonymous().await;
    let client = reqwest::Client::new();
    create_bucket(&client, &server.base_url, "tag-bucket").await;

    // Upload object
    client
        .put(format!("{}/tag-bucket/file.txt", server.base_url))
        .body("data")
        .send()
        .await
        .unwrap();

    // Get tags (should be empty)
    let resp = client
        .get(format!("{}/tag-bucket/file.txt?tagging", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(body.contains("<TagSet/>") || body.contains("<TagSet></TagSet>"));

    // Put tags
    let tag_xml = r#"<Tagging><TagSet><Tag><Key>env</Key><Value>prod</Value></Tag><Tag><Key>team</Key><Value>eng</Value></Tag></TagSet></Tagging>"#;
    let resp = client
        .put(format!("{}/tag-bucket/file.txt?tagging", server.base_url))
        .body(tag_xml)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Get tags
    let resp = client
        .get(format!("{}/tag-bucket/file.txt?tagging", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(body.contains("<Key>env</Key>"));
    assert!(body.contains("<Value>prod</Value>"));

    // Delete tags
    let resp = client
        .delete(format!("{}/tag-bucket/file.txt?tagging", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);

    // Verify tags are gone
    let resp = client
        .get(format!("{}/tag-bucket/file.txt?tagging", server.base_url))
        .send()
        .await
        .unwrap();
    let body = resp.text().await.unwrap();
    assert!(body.contains("<TagSet/>") || body.contains("<TagSet></TagSet>"));
}

#[tokio::test]
async fn test_get_object_returns_tagging_count() {
    let server = TestServer::start_anonymous().await;
    let client = reqwest::Client::new();
    create_bucket(&client, &server.base_url, "tagcount-bucket").await;

    client
        .put(format!("{}/tagcount-bucket/file.txt", server.base_url))
        .body("data")
        .send()
        .await
        .unwrap();

    // No tags — no x-amz-tagging-count header
    let resp = client
        .get(format!("{}/tagcount-bucket/file.txt", server.base_url))
        .send()
        .await
        .unwrap();
    assert!(resp.headers().get("x-amz-tagging-count").is_none());

    // Add tags
    let tag_xml = r#"<Tagging><TagSet><Tag><Key>a</Key><Value>1</Value></Tag></TagSet></Tagging>"#;
    client
        .put(format!("{}/tagcount-bucket/file.txt?tagging", server.base_url))
        .body(tag_xml)
        .send()
        .await
        .unwrap();

    // Now should have tagging count header
    let resp = client
        .get(format!("{}/tagcount-bucket/file.txt", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.headers().get("x-amz-tagging-count").unwrap().to_str().unwrap(),
        "1"
    );
}

// --- CopyObject tests ---

#[tokio::test]
async fn test_copy_object_same_bucket() {
    let server = TestServer::start_anonymous().await;
    let client = reqwest::Client::new();
    create_bucket(&client, &server.base_url, "copy-bucket").await;

    client
        .put(format!("{}/copy-bucket/src.txt", server.base_url))
        .body("original")
        .send()
        .await
        .unwrap();

    let resp = client
        .put(format!("{}/copy-bucket/dst.txt", server.base_url))
        .header("x-amz-copy-source", "/copy-bucket/src.txt")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(body.contains("<CopyObjectResult"));
    assert!(body.contains("<ETag>"));

    // Verify copy
    let resp = client
        .get(format!("{}/copy-bucket/dst.txt", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert_eq!(body, "original");
}

#[tokio::test]
async fn test_copy_object_cross_bucket() {
    let server = TestServer::start_anonymous().await;
    let client = reqwest::Client::new();
    create_bucket(&client, &server.base_url, "src-bucket").await;
    create_bucket(&client, &server.base_url, "dst-bucket").await;

    client
        .put(format!("{}/src-bucket/file.txt", server.base_url))
        .body("cross bucket copy")
        .send()
        .await
        .unwrap();

    let resp = client
        .put(format!("{}/dst-bucket/file.txt", server.base_url))
        .header("x-amz-copy-source", "/src-bucket/file.txt")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let resp = client
        .get(format!("{}/dst-bucket/file.txt", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.text().await.unwrap(), "cross bucket copy");
}

#[tokio::test]
async fn test_copy_nonexistent_source() {
    let server = TestServer::start_anonymous().await;
    let client = reqwest::Client::new();
    create_bucket(&client, &server.base_url, "copy-err").await;

    let resp = client
        .put(format!("{}/copy-err/dst.txt", server.base_url))
        .header("x-amz-copy-source", "/copy-err/nonexistent.txt")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

// --- DeleteObjects (batch delete) tests ---

#[tokio::test]
async fn test_delete_objects_basic() {
    let server = TestServer::start_anonymous().await;
    let client = reqwest::Client::new();
    create_bucket(&client, &server.base_url, "batch-del").await;

    for key in ["a.txt", "b.txt", "c.txt"] {
        client
            .put(format!("{}/batch-del/{}", server.base_url, key))
            .body("data")
            .send()
            .await
            .unwrap();
    }

    let delete_xml = r#"<Delete><Object><Key>a.txt</Key></Object><Object><Key>b.txt</Key></Object></Delete>"#;
    let resp = client
        .post(format!("{}/batch-del?delete", server.base_url))
        .body(delete_xml)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(body.contains("<Deleted>"));
    assert!(body.contains("<Key>a.txt</Key>"));
    assert!(body.contains("<Key>b.txt</Key>"));

    // Verify deleted
    let resp = client
        .get(format!("{}/batch-del/a.txt", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);

    // c.txt should still exist
    let resp = client
        .get(format!("{}/batch-del/c.txt", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_delete_objects_nonexistent_keys() {
    let server = TestServer::start_anonymous().await;
    let client = reqwest::Client::new();
    create_bucket(&client, &server.base_url, "batch-del2").await;

    let delete_xml = r#"<Delete><Object><Key>nope1.txt</Key></Object><Object><Key>nope2.txt</Key></Object></Delete>"#;
    let resp = client
        .post(format!("{}/batch-del2?delete", server.base_url))
        .body(delete_xml)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    // Nonexistent keys are treated as successful deletes
    assert!(body.contains("<Deleted>"));
}
