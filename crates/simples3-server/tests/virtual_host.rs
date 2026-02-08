mod common;

use common::TestServer;

#[tokio::test]
async fn test_virtual_host_head_bucket() {
    let server = TestServer::start_anonymous().await;

    server.metadata.create_bucket("vhost-bucket").unwrap();

    let client = reqwest::Client::new();
    let resp = client
        .head(format!("http://{}/", server.addr))
        .header("host", format!("vhost-bucket.s3.localhost:{}", server.addr.port()))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_virtual_host_put_and_get() {
    let server = TestServer::start_anonymous().await;

    server.metadata.create_bucket("vh-bucket").unwrap();

    let client = reqwest::Client::new();

    // PUT via virtual-host style
    let resp = client
        .put(format!("http://{}/mykey.txt", server.addr))
        .header("host", format!("vh-bucket.s3.localhost:{}", server.addr.port()))
        .body("virtual host data")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // GET via path-style
    let resp = client
        .get(format!("{}/vh-bucket/mykey.txt", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert_eq!(body, "virtual host data");
}
