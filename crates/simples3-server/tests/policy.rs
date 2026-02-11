mod common;

use common::TestServer;

#[tokio::test]
async fn test_policy_crud() {
    let server = TestServer::start_anonymous().await;
    let client = reqwest::Client::new();

    // Create bucket
    let resp = client
        .put(format!("{}/policy-bucket", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // PUT bucket policy
    let policy_json = r#"{
        "Version": "2012-10-17",
        "Statement": [
            {
                "Sid": "AllowAnonymousGet",
                "Effect": "Allow",
                "Principal": "*",
                "Action": "s3:GetObject",
                "Resource": "arn:aws:s3:::policy-bucket/*"
            }
        ]
    }"#;

    let resp = client
        .put(format!("{}/policy-bucket?policy", server.base_url))
        .body(policy_json)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);

    // GET bucket policy
    let resp = client
        .get(format!("{}/policy-bucket?policy", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(body.contains("AllowAnonymousGet"));
    assert!(body.contains("s3:GetObject"));

    // DELETE bucket policy
    let resp = client
        .delete(format!("{}/policy-bucket?policy", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);

    // GET should now 404
    let resp = client
        .get(format!("{}/policy-bucket?policy", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn test_policy_anonymous_access_via_policy() {
    let server = TestServer::start().await;
    let client = reqwest::Client::new();

    // Create bucket via metadata store
    server.metadata.create_bucket("policy-anon").unwrap();

    // Store an object directly via metadata + filestore
    server
        .metadata
        .put_object_meta(&simples3_core::s3::types::ObjectMeta {
            bucket: "policy-anon".into(),
            key: "public-file.txt".into(),
            size: 12,
            etag: "abc".into(),
            content_type: "text/plain".into(),
            last_modified: chrono::Utc::now(),
            public: false,
        })
        .unwrap();

    // Without policy, anonymous HEAD should be denied
    let resp = client
        .head(format!("{}/policy-anon/public-file.txt", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);

    // Set bucket policy allowing anonymous HeadObject/GetObject via metadata store
    let policy = simples3_core::s3::types::BucketPolicy {
        version: "2012-10-17".into(),
        statements: vec![simples3_core::s3::types::PolicyStatement {
            sid: Some("AllowAnonymousRead".into()),
            effect: simples3_core::s3::types::PolicyEffect::Allow,
            principal: simples3_core::s3::types::PolicyPrincipal::Wildcard("*".into()),
            action: simples3_core::s3::types::OneOrMany::Many(vec![
                "s3:GetObject".into(),
                "s3:HeadObject".into(),
            ]),
            resource: simples3_core::s3::types::OneOrMany::One(
                "arn:aws:s3:::policy-anon/*".into(),
            ),
            condition: None,
        }],
    };
    server
        .metadata
        .put_bucket_policy("policy-anon", &policy)
        .unwrap();

    // Anonymous HEAD should now be allowed by policy
    let resp = client
        .head(format!("{}/policy-anon/public-file.txt", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Anonymous PUT should still be denied
    let resp = client
        .put(format!("{}/policy-anon/other.txt", server.base_url))
        .body("nope")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);
}

#[tokio::test]
async fn test_policy_explicit_deny() {
    let server = TestServer::start().await;
    let client = reqwest::Client::new();

    // Create bucket via metadata store
    server.metadata.create_bucket("deny-bucket").unwrap();

    // Store an object
    server
        .metadata
        .put_object_meta(&simples3_core::s3::types::ObjectMeta {
            bucket: "deny-bucket".into(),
            key: "secret.txt".into(),
            size: 9,
            etag: "xyz".into(),
            content_type: "text/plain".into(),
            last_modified: chrono::Utc::now(),
            public: false,
        })
        .unwrap();

    // Set policy: Allow anonymous HeadObject but explicitly deny HeadObject for a specific key
    // This tests deny-trumps-allow behavior
    let policy = simples3_core::s3::types::BucketPolicy {
        version: "2012-10-17".into(),
        statements: vec![
            simples3_core::s3::types::PolicyStatement {
                sid: Some("AllowHead".into()),
                effect: simples3_core::s3::types::PolicyEffect::Allow,
                principal: simples3_core::s3::types::PolicyPrincipal::Wildcard("*".into()),
                action: simples3_core::s3::types::OneOrMany::One("s3:HeadObject".into()),
                resource: simples3_core::s3::types::OneOrMany::One(
                    "arn:aws:s3:::deny-bucket/*".into(),
                ),
                condition: None,
            },
            simples3_core::s3::types::PolicyStatement {
                sid: Some("DenyHead".into()),
                effect: simples3_core::s3::types::PolicyEffect::Deny,
                principal: simples3_core::s3::types::PolicyPrincipal::Wildcard("*".into()),
                action: simples3_core::s3::types::OneOrMany::One("s3:HeadObject".into()),
                resource: simples3_core::s3::types::OneOrMany::One(
                    "arn:aws:s3:::deny-bucket/secret.txt".into(),
                ),
                condition: None,
            },
        ],
    };
    server
        .metadata
        .put_bucket_policy("deny-bucket", &policy)
        .unwrap();

    // Anonymous HEAD on secret.txt should be denied (explicit deny overrides allow)
    let resp = client
        .head(format!("{}/deny-bucket/secret.txt", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);
}
