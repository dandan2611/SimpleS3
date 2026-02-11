mod common;

use common::TestServer;

#[tokio::test]
async fn test_lifecycle_crud() {
    let server = TestServer::start_anonymous().await;
    let client = reqwest::Client::new();

    // Create bucket
    let resp = client
        .put(format!("{}/lifecycle-bucket", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // PUT lifecycle configuration
    let lifecycle_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<LifecycleConfiguration>
    <Rule>
        <ID>expire-logs</ID>
        <Filter><Prefix>logs/</Prefix></Filter>
        <Status>Enabled</Status>
        <Expiration><Days>30</Days></Expiration>
    </Rule>
</LifecycleConfiguration>"#;

    let resp = client
        .put(format!("{}/lifecycle-bucket?lifecycle", server.base_url))
        .body(lifecycle_xml)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // GET lifecycle configuration
    let resp = client
        .get(format!("{}/lifecycle-bucket?lifecycle", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(body.contains("<ID>expire-logs</ID>"));
    assert!(body.contains("<Prefix>logs/</Prefix>"));
    assert!(body.contains("<Days>30</Days>"));
    assert!(body.contains("<Status>Enabled</Status>"));

    // DELETE lifecycle configuration
    let resp = client
        .delete(format!("{}/lifecycle-bucket?lifecycle", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);

    // GET should now 404
    let resp = client
        .get(format!("{}/lifecycle-bucket?lifecycle", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn test_lifecycle_nonexistent_bucket() {
    let server = TestServer::start_anonymous().await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("{}/no-such-bucket?lifecycle", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}
