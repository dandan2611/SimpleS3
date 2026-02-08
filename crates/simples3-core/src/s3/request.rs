use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum S3Operation {
    ListBuckets,
    CreateBucket { bucket: String },
    DeleteBucket { bucket: String },
    HeadBucket { bucket: String },
    ListObjectsV2 { bucket: String },
    PutObject { bucket: String, key: String },
    GetObject { bucket: String, key: String },
    HeadObject { bucket: String, key: String },
    DeleteObject { bucket: String, key: String },
    CreateMultipartUpload { bucket: String, key: String },
    UploadPart { bucket: String, key: String, upload_id: String, part_number: u32 },
    CompleteMultipartUpload { bucket: String, key: String, upload_id: String },
    AbortMultipartUpload { bucket: String, key: String, upload_id: String },
    ListParts { bucket: String, key: String, upload_id: String },
    PutObjectTagging { bucket: String, key: String },
    GetObjectTagging { bucket: String, key: String },
    DeleteObjectTagging { bucket: String, key: String },
    DeleteObjects { bucket: String },
}

impl S3Operation {
    pub fn bucket(&self) -> Option<&str> {
        match self {
            S3Operation::ListBuckets => None,
            S3Operation::CreateBucket { bucket }
            | S3Operation::DeleteBucket { bucket }
            | S3Operation::HeadBucket { bucket }
            | S3Operation::ListObjectsV2 { bucket } => Some(bucket),
            S3Operation::PutObject { bucket, .. }
            | S3Operation::GetObject { bucket, .. }
            | S3Operation::HeadObject { bucket, .. }
            | S3Operation::DeleteObject { bucket, .. }
            | S3Operation::CreateMultipartUpload { bucket, .. }
            | S3Operation::UploadPart { bucket, .. }
            | S3Operation::CompleteMultipartUpload { bucket, .. }
            | S3Operation::AbortMultipartUpload { bucket, .. }
            | S3Operation::ListParts { bucket, .. }
            | S3Operation::PutObjectTagging { bucket, .. }
            | S3Operation::GetObjectTagging { bucket, .. }
            | S3Operation::DeleteObjectTagging { bucket, .. } => Some(bucket),
            S3Operation::DeleteObjects { bucket } => Some(bucket),
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            S3Operation::ListBuckets => "ListBuckets",
            S3Operation::CreateBucket { .. } => "CreateBucket",
            S3Operation::DeleteBucket { .. } => "DeleteBucket",
            S3Operation::HeadBucket { .. } => "HeadBucket",
            S3Operation::ListObjectsV2 { .. } => "ListObjectsV2",
            S3Operation::PutObject { .. } => "PutObject",
            S3Operation::GetObject { .. } => "GetObject",
            S3Operation::HeadObject { .. } => "HeadObject",
            S3Operation::DeleteObject { .. } => "DeleteObject",
            S3Operation::CreateMultipartUpload { .. } => "CreateMultipartUpload",
            S3Operation::UploadPart { .. } => "UploadPart",
            S3Operation::CompleteMultipartUpload { .. } => "CompleteMultipartUpload",
            S3Operation::AbortMultipartUpload { .. } => "AbortMultipartUpload",
            S3Operation::ListParts { .. } => "ListParts",
            S3Operation::PutObjectTagging { .. } => "PutObjectTagging",
            S3Operation::GetObjectTagging { .. } => "GetObjectTagging",
            S3Operation::DeleteObjectTagging { .. } => "DeleteObjectTagging",
            S3Operation::DeleteObjects { .. } => "DeleteObjects",
        }
    }

    pub fn is_read_only(&self) -> bool {
        matches!(
            self,
            S3Operation::ListBuckets
                | S3Operation::HeadBucket { .. }
                | S3Operation::ListObjectsV2 { .. }
                | S3Operation::GetObject { .. }
                | S3Operation::HeadObject { .. }
                | S3Operation::ListParts { .. }
                | S3Operation::GetObjectTagging { .. }
        )
    }
}

pub fn parse_s3_operation(
    method: &http::Method,
    path: &str,
    query: &HashMap<String, String>,
) -> Option<S3Operation> {
    let path = path.trim_start_matches('/');

    // Root path: list buckets
    if path.is_empty() {
        if method == http::Method::GET {
            return Some(S3Operation::ListBuckets);
        }
        return None;
    }

    // Split into bucket and key
    let (bucket, key) = match path.find('/') {
        Some(idx) => (&path[..idx], &path[idx + 1..]),
        None => (path, ""),
    };

    let bucket = bucket.to_string();

    // Bucket-level operations (no key)
    if key.is_empty() {
        if query.contains_key("delete") && *method == http::Method::POST {
            return Some(S3Operation::DeleteObjects { bucket });
        }
        return match *method {
            http::Method::PUT => Some(S3Operation::CreateBucket { bucket }),
            http::Method::DELETE => Some(S3Operation::DeleteBucket { bucket }),
            http::Method::HEAD => Some(S3Operation::HeadBucket { bucket }),
            http::Method::GET => {
                if query.contains_key("list-type") {
                    Some(S3Operation::ListObjectsV2 { bucket })
                } else {
                    // Default GET on bucket is also list objects
                    Some(S3Operation::ListObjectsV2 { bucket })
                }
            }
            _ => None,
        };
    }

    let key = key.to_string();

    // Multipart operations
    if query.contains_key("uploads") && method == http::Method::POST {
        return Some(S3Operation::CreateMultipartUpload { bucket, key });
    }

    if let Some(upload_id) = query.get("uploadId").cloned() {
        return match *method {
            http::Method::PUT => {
                let part_number: u32 = query
                    .get("partNumber")
                    .and_then(|p| p.parse().ok())
                    .unwrap_or(0);
                Some(S3Operation::UploadPart {
                    bucket,
                    key,
                    upload_id,
                    part_number,
                })
            }
            http::Method::POST => Some(S3Operation::CompleteMultipartUpload {
                bucket,
                key,
                upload_id,
            }),
            http::Method::DELETE => Some(S3Operation::AbortMultipartUpload {
                bucket,
                key,
                upload_id,
            }),
            http::Method::GET => Some(S3Operation::ListParts {
                bucket,
                key,
                upload_id,
            }),
            _ => None,
        };
    }

    // Tagging operations
    if query.contains_key("tagging") {
        return match *method {
            http::Method::PUT => Some(S3Operation::PutObjectTagging { bucket, key }),
            http::Method::GET => Some(S3Operation::GetObjectTagging { bucket, key }),
            http::Method::DELETE => Some(S3Operation::DeleteObjectTagging { bucket, key }),
            _ => None,
        };
    }

    // Object operations
    match *method {
        http::Method::PUT => Some(S3Operation::PutObject { bucket, key }),
        http::Method::GET => Some(S3Operation::GetObject { bucket, key }),
        http::Method::HEAD => Some(S3Operation::HeadObject { bucket, key }),
        http::Method::DELETE => Some(S3Operation::DeleteObject { bucket, key }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn test_parse_list_buckets() {
        let op = parse_s3_operation(&http::Method::GET, "/", &HashMap::new());
        assert_eq!(op, Some(S3Operation::ListBuckets));
    }

    #[test]
    fn test_parse_put_object() {
        let op = parse_s3_operation(&http::Method::PUT, "/mybucket/mykey.txt", &HashMap::new());
        assert_eq!(
            op,
            Some(S3Operation::PutObject {
                bucket: "mybucket".into(),
                key: "mykey.txt".into()
            })
        );
    }

    #[test]
    fn test_parse_list_objects() {
        let op = parse_s3_operation(
            &http::Method::GET,
            "/mybucket",
            &query(&[("list-type", "2")]),
        );
        assert_eq!(op, Some(S3Operation::ListObjectsV2 { bucket: "mybucket".into() }));
    }

    #[test]
    fn test_parse_multipart_create() {
        let op = parse_s3_operation(
            &http::Method::POST,
            "/mybucket/mykey",
            &query(&[("uploads", "")]),
        );
        assert_eq!(
            op,
            Some(S3Operation::CreateMultipartUpload {
                bucket: "mybucket".into(),
                key: "mykey".into()
            })
        );
    }

    #[test]
    fn test_parse_upload_part() {
        let op = parse_s3_operation(
            &http::Method::PUT,
            "/mybucket/mykey",
            &query(&[("partNumber", "1"), ("uploadId", "abc123")]),
        );
        assert_eq!(
            op,
            Some(S3Operation::UploadPart {
                bucket: "mybucket".into(),
                key: "mykey".into(),
                upload_id: "abc123".into(),
                part_number: 1,
            })
        );
    }

    #[test]
    fn test_parse_put_object_tagging() {
        let op = parse_s3_operation(
            &http::Method::PUT,
            "/mybucket/mykey",
            &query(&[("tagging", "")]),
        );
        assert_eq!(
            op,
            Some(S3Operation::PutObjectTagging {
                bucket: "mybucket".into(),
                key: "mykey".into()
            })
        );
    }

    #[test]
    fn test_parse_get_object_tagging() {
        let op = parse_s3_operation(
            &http::Method::GET,
            "/mybucket/mykey",
            &query(&[("tagging", "")]),
        );
        assert_eq!(
            op,
            Some(S3Operation::GetObjectTagging {
                bucket: "mybucket".into(),
                key: "mykey".into()
            })
        );
    }

    #[test]
    fn test_parse_delete_object_tagging() {
        let op = parse_s3_operation(
            &http::Method::DELETE,
            "/mybucket/mykey",
            &query(&[("tagging", "")]),
        );
        assert_eq!(
            op,
            Some(S3Operation::DeleteObjectTagging {
                bucket: "mybucket".into(),
                key: "mykey".into()
            })
        );
    }

    #[test]
    fn test_parse_delete_objects() {
        let op = parse_s3_operation(
            &http::Method::POST,
            "/mybucket",
            &query(&[("delete", "")]),
        );
        assert_eq!(
            op,
            Some(S3Operation::DeleteObjects {
                bucket: "mybucket".into()
            })
        );
    }

    #[test]
    fn test_parse_nested_key() {
        let op = parse_s3_operation(&http::Method::GET, "/mybucket/a/b/c.txt", &HashMap::new());
        assert_eq!(
            op,
            Some(S3Operation::GetObject {
                bucket: "mybucket".into(),
                key: "a/b/c.txt".into()
            })
        );
    }
}
