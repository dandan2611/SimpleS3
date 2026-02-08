mod common;

use common::TestServer;

// Note: multipart tests require authenticated requests in the real flow,
// but since our test server has auth middleware that blocks unauthenticated requests,
// we test multipart via direct metadata/filestore for correctness of the core logic.
// Full integration with SigV4 signing would require a signing helper.

#[tokio::test]
async fn test_multipart_core_lifecycle() {
    use chrono::Utc;
    use simples3_core::s3::types::{MultipartUpload, PartInfo};
    use simples3_core::storage::FileStore;

    let server = TestServer::start_anonymous().await;

    let upload_id = "test-upload-1";
    let upload = MultipartUpload {
        upload_id: upload_id.into(),
        bucket: "mp-bucket".into(),
        key: "large-file.bin".into(),
        created: Utc::now(),
        parts: vec![],
    };

    server.metadata.create_bucket("mp-bucket").unwrap();
    server
        .metadata
        .create_multipart_upload(&upload)
        .unwrap();

    // Add parts via metadata
    server
        .metadata
        .add_part_to_upload(
            upload_id,
            PartInfo {
                part_number: 1,
                etag: "etag1".into(),
                size: 100,
                last_modified: Utc::now(),
            },
        )
        .unwrap();

    server
        .metadata
        .add_part_to_upload(
            upload_id,
            PartInfo {
                part_number: 2,
                etag: "etag2".into(),
                size: 200,
                last_modified: Utc::now(),
            },
        )
        .unwrap();

    // List parts
    let fetched = server.metadata.get_multipart_upload(upload_id).unwrap();
    assert_eq!(fetched.parts.len(), 2);
    assert_eq!(fetched.parts[0].part_number, 1);
    assert_eq!(fetched.parts[1].part_number, 2);

    // Abort / cleanup
    server
        .metadata
        .delete_multipart_upload(upload_id)
        .unwrap();
    assert!(server.metadata.get_multipart_upload(upload_id).is_err());
}
