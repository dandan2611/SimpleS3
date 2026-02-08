mod common;

use chrono::Utc;
use common::TestServer;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC key");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

fn signing_key(secret: &str, date: &str, region: &str) -> Vec<u8> {
    let k_date = hmac_sha256(format!("AWS4{}", secret).as_bytes(), date.as_bytes());
    let k_region = hmac_sha256(&k_date, region.as_bytes());
    let k_service = hmac_sha256(&k_region, b"s3");
    hmac_sha256(&k_service, b"aws4_request")
}

fn generate_presigned_url(
    method: &str,
    base_url: &str,
    path: &str,
    access_key: &str,
    secret_key: &str,
    region: &str,
    expires_secs: u64,
    host: &str,
) -> String {
    let now = Utc::now();
    let date = now.format("%Y%m%d").to_string();
    let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();
    let credential = format!("{}/{}/{}/s3/aws4_request", access_key, date, region);

    let signed_headers = "host";

    // Build canonical query string (without signature, sorted)
    let mut params = vec![
        ("X-Amz-Algorithm".to_string(), "AWS4-HMAC-SHA256".to_string()),
        (
            "X-Amz-Credential".to_string(),
            percent_encoding::utf8_percent_encode(&credential, percent_encoding::NON_ALPHANUMERIC)
                .to_string(),
        ),
        ("X-Amz-Date".to_string(), amz_date.clone()),
        ("X-Amz-Expires".to_string(), expires_secs.to_string()),
        (
            "X-Amz-SignedHeaders".to_string(),
            signed_headers.to_string(),
        ),
    ];
    params.sort_by(|a, b| a.0.cmp(&b.0));
    let canonical_query: String = params
        .iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join("&");

    // Build canonical request
    let canonical_headers = format!("host:{}\n", host);
    let canonical_request = format!(
        "{}\n{}\n{}\n{}\n{}\n{}",
        method, path, canonical_query, canonical_headers, signed_headers, "UNSIGNED-PAYLOAD"
    );

    let hash_canon = hex::encode(Sha256::digest(canonical_request.as_bytes()));
    let scope = format!("{}/{}/s3/aws4_request", date, region);
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{}\n{}\n{}",
        amz_date, scope, hash_canon
    );

    let key = signing_key(secret_key, &date, region);
    let signature = hex::encode(hmac_sha256(&key, string_to_sign.as_bytes()));

    format!(
        "{}{}?{}&X-Amz-Signature={}",
        base_url, path, canonical_query, signature
    )
}

async fn create_bucket(client: &reqwest::Client, base_url: &str, name: &str) {
    client
        .put(format!("{}/{}", base_url, name))
        .send()
        .await
        .unwrap();
}

#[tokio::test]
async fn test_presigned_get_object() {
    let server = TestServer::start().await;
    let client = reqwest::Client::new();

    // Create bucket and upload object using anonymous-free helper
    // We need to use presigned for both or use the non-anonymous server
    // Actually let's start an anonymous server just for setup, then test presigned separately
    let anon_server = TestServer::start_anonymous().await;
    let anon_client = reqwest::Client::new();
    create_bucket(&anon_client, &anon_server.base_url, "presign-bucket").await;
    anon_client
        .put(format!("{}/presign-bucket/hello.txt", anon_server.base_url))
        .body("presigned content")
        .send()
        .await
        .unwrap();

    // Now test presigned GET on the anonymous server (which also accepts presigned)
    let host = anon_server.addr.to_string();
    let url = generate_presigned_url(
        "GET",
        &anon_server.base_url,
        "/presign-bucket/hello.txt",
        "TESTAKID",
        "TESTSECRET",
        "us-east-1",
        300,
        &host,
    );

    let resp = client.get(&url).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert_eq!(body, "presigned content");
}

#[tokio::test]
async fn test_presigned_put_object() {
    let server = TestServer::start().await;
    let client = reqwest::Client::new();

    // Use the non-anonymous server: create bucket via presigned PUT won't work easily
    // So let's just use the metadata store directly
    server.metadata.create_bucket("presign-put").unwrap();
    // Need filestore bucket dir too
    let host = server.addr.to_string();

    let url = generate_presigned_url(
        "PUT",
        &server.base_url,
        "/presign-put/uploaded.txt",
        "TESTAKID",
        "TESTSECRET",
        "us-east-1",
        300,
        &host,
    );

    let resp = client
        .put(&url)
        .body("presigned upload")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Verify by reading back (via presigned GET)
    let get_url = generate_presigned_url(
        "GET",
        &server.base_url,
        "/presign-put/uploaded.txt",
        "TESTAKID",
        "TESTSECRET",
        "us-east-1",
        300,
        &host,
    );
    let resp = client.get(&get_url).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert_eq!(body, "presigned upload");
}

#[tokio::test]
async fn test_presigned_expired() {
    let server = TestServer::start().await;
    let client = reqwest::Client::new();
    server.metadata.create_bucket("presign-exp").unwrap();

    let host = server.addr.to_string();

    // Generate a URL with 0 expiry — it will be immediately expired
    // We use a past date to simulate expiration
    let now = chrono::Utc::now() - chrono::Duration::seconds(600);
    let date = now.format("%Y%m%d").to_string();
    let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();
    let credential = format!("TESTAKID/{}/us-east-1/s3/aws4_request", date);
    let path = "/presign-exp/file.txt";

    let mut params = vec![
        ("X-Amz-Algorithm".to_string(), "AWS4-HMAC-SHA256".to_string()),
        (
            "X-Amz-Credential".to_string(),
            percent_encoding::utf8_percent_encode(&credential, percent_encoding::NON_ALPHANUMERIC)
                .to_string(),
        ),
        ("X-Amz-Date".to_string(), amz_date.clone()),
        ("X-Amz-Expires".to_string(), "60".to_string()),
        ("X-Amz-SignedHeaders".to_string(), "host".to_string()),
    ];
    params.sort_by(|a, b| a.0.cmp(&b.0));
    let canonical_query: String = params
        .iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join("&");

    let canonical_headers = format!("host:{}\n", host);
    let canonical_request = format!(
        "GET\n{}\n{}\n{}\nhost\nUNSIGNED-PAYLOAD",
        path, canonical_query, canonical_headers
    );

    let hash_canon = hex::encode(Sha256::digest(canonical_request.as_bytes()));
    let scope = format!("{}/us-east-1/s3/aws4_request", date);
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{}\n{}\n{}",
        amz_date, scope, hash_canon
    );

    let key = signing_key("TESTSECRET", &date, "us-east-1");
    let signature = hex::encode(hmac_sha256(&key, string_to_sign.as_bytes()));

    let url = format!(
        "{}{}?{}&X-Amz-Signature={}",
        server.base_url, path, canonical_query, signature
    );

    let resp = client.get(&url).send().await.unwrap();
    assert_eq!(resp.status(), 403);
}
