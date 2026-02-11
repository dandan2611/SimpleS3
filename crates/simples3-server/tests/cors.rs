mod common;

use common::TestServer;

#[tokio::test]
async fn test_cors_crud() {
    let server = TestServer::start_anonymous().await;
    let client = reqwest::Client::new();

    // Create bucket
    client
        .put(format!("{}/cors-test-bkt", server.base_url))
        .send()
        .await
        .unwrap();

    // No CORS config initially
    let resp = client
        .get(format!("{}/cors-test-bkt?cors", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);

    // Put CORS configuration
    let cors_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<CORSConfiguration xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
  <CORSRule>
    <ID>test-rule</ID>
    <AllowedOrigin>https://example.com</AllowedOrigin>
    <AllowedMethod>GET</AllowedMethod>
    <AllowedMethod>PUT</AllowedMethod>
    <AllowedHeader>*</AllowedHeader>
    <ExposeHeader>x-amz-request-id</ExposeHeader>
    <MaxAgeSeconds>3600</MaxAgeSeconds>
  </CORSRule>
</CORSConfiguration>"#;

    let resp = client
        .put(format!("{}/cors-test-bkt?cors", server.base_url))
        .body(cors_xml)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Get CORS configuration
    let resp = client
        .get(format!("{}/cors-test-bkt?cors", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(body.contains("<AllowedOrigin>https://example.com</AllowedOrigin>"));
    assert!(body.contains("<AllowedMethod>GET</AllowedMethod>"));
    assert!(body.contains("<ID>test-rule</ID>"));
    assert!(body.contains("<MaxAgeSeconds>3600</MaxAgeSeconds>"));

    // Delete CORS configuration
    let resp = client
        .delete(format!("{}/cors-test-bkt?cors", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);

    // Verify deleted
    let resp = client
        .get(format!("{}/cors-test-bkt?cors", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn test_cors_nonexistent_bucket() {
    let server = TestServer::start_anonymous().await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("{}/nonexistent-bkt?cors", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn test_cors_preflight_per_bucket() {
    let server = TestServer::start_anonymous().await;
    let client = reqwest::Client::new();

    // Create bucket
    client
        .put(format!("{}/cors-pf-bkt", server.base_url))
        .send()
        .await
        .unwrap();

    // Put CORS configuration
    let cors_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<CORSConfiguration xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
  <CORSRule>
    <AllowedOrigin>https://myapp.com</AllowedOrigin>
    <AllowedMethod>GET</AllowedMethod>
    <AllowedMethod>PUT</AllowedMethod>
    <AllowedHeader>content-type</AllowedHeader>
    <MaxAgeSeconds>600</MaxAgeSeconds>
  </CORSRule>
</CORSConfiguration>"#;

    client
        .put(format!("{}/cors-pf-bkt?cors", server.base_url))
        .body(cors_xml)
        .send()
        .await
        .unwrap();

    // Send preflight OPTIONS request
    let resp = client
        .request(reqwest::Method::OPTIONS, format!("{}/cors-pf-bkt/test.txt", server.base_url))
        .header("origin", "https://myapp.com")
        .header("access-control-request-method", "PUT")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(
        resp.headers().get("access-control-allow-origin").unwrap(),
        "https://myapp.com"
    );
    let allow_methods = resp
        .headers()
        .get("access-control-allow-methods")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(allow_methods.contains("GET"));
    assert!(allow_methods.contains("PUT"));
    assert_eq!(
        resp.headers().get("access-control-max-age").unwrap(),
        "600"
    );
}

#[tokio::test]
async fn test_cors_response_headers_on_get() {
    let server = TestServer::start_anonymous().await;
    let client = reqwest::Client::new();

    // Create bucket
    client
        .put(format!("{}/cors-hdr-bkt", server.base_url))
        .send()
        .await
        .unwrap();

    // Put CORS configuration
    let cors_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<CORSConfiguration xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
  <CORSRule>
    <AllowedOrigin>https://webapp.com</AllowedOrigin>
    <AllowedMethod>GET</AllowedMethod>
    <ExposeHeader>x-amz-request-id</ExposeHeader>
  </CORSRule>
</CORSConfiguration>"#;

    client
        .put(format!("{}/cors-hdr-bkt?cors", server.base_url))
        .body(cors_xml)
        .send()
        .await
        .unwrap();

    // PUT an object
    client
        .put(format!("{}/cors-hdr-bkt/file.txt", server.base_url))
        .body("hello cors")
        .send()
        .await
        .unwrap();

    // GET with matching Origin header
    let resp = client
        .get(format!("{}/cors-hdr-bkt/file.txt", server.base_url))
        .header("origin", "https://webapp.com")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(
        resp.headers().get("access-control-allow-origin").unwrap(),
        "https://webapp.com"
    );
    assert!(resp
        .headers()
        .get("access-control-expose-headers")
        .unwrap()
        .to_str()
        .unwrap()
        .contains("x-amz-request-id"));
}
