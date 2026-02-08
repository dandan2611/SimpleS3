use quick_xml::Writer;
use quick_xml::events::BytesText;
use std::collections::HashMap;
use std::io::Cursor;

use crate::s3::types::{
    BucketMeta, ListObjectsV2Response, MultipartUpload, ObjectMeta, PartInfo,
};

const S3_XMLNS: &str = "http://s3.amazonaws.com/doc/2006-03-01/";

fn xml_header() -> &'static str {
    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>"
}

pub fn list_buckets_xml(owner_id: &str, buckets: &[BucketMeta]) -> String {
    let mut writer = Writer::new(Cursor::new(Vec::new()));
    writer
        .create_element("ListAllMyBucketsResult")
        .with_attribute(("xmlns", S3_XMLNS))
        .write_inner_content(|w| {
            w.create_element("Owner")
                .write_inner_content(|w| {
                    w.create_element("ID")
                        .write_text_content(BytesText::new(owner_id))?;
                    w.create_element("DisplayName")
                        .write_text_content(BytesText::new(owner_id))?;
                    Ok(())
                })?;
            w.create_element("Buckets")
                .write_inner_content(|w| {
                    for b in buckets {
                        w.create_element("Bucket")
                            .write_inner_content(|w| {
                                w.create_element("Name")
                                    .write_text_content(BytesText::new(&b.name))?;
                                w.create_element("CreationDate")
                                    .write_text_content(BytesText::new(
                                        &b.creation_date.to_rfc3339(),
                                    ))?;
                                Ok(())
                            })?;
                    }
                    Ok(())
                })?;
            Ok(())
        })
        .unwrap();
    let bytes = writer.into_inner().into_inner();
    format!("{}{}", xml_header(), String::from_utf8(bytes).unwrap())
}

pub fn list_objects_v2_xml(resp: &ListObjectsV2Response) -> String {
    let mut writer = Writer::new(Cursor::new(Vec::new()));
    writer
        .create_element("ListBucketResult")
        .with_attribute(("xmlns", S3_XMLNS))
        .write_inner_content(|w| {
            w.create_element("Name")
                .write_text_content(BytesText::new(&resp.name))?;
            w.create_element("Prefix")
                .write_text_content(BytesText::new(&resp.prefix))?;
            w.create_element("MaxKeys")
                .write_text_content(BytesText::new(&resp.max_keys.to_string()))?;
            w.create_element("KeyCount")
                .write_text_content(BytesText::new(&resp.key_count.to_string()))?;
            w.create_element("IsTruncated")
                .write_text_content(BytesText::new(&resp.is_truncated.to_string()))?;
            if !resp.delimiter.is_empty() {
                w.create_element("Delimiter")
                    .write_text_content(BytesText::new(&resp.delimiter))?;
            }
            if let Some(ref token) = resp.next_continuation_token {
                w.create_element("NextContinuationToken")
                    .write_text_content(BytesText::new(token))?;
            }
            for obj in &resp.contents {
                write_object_xml(w, obj)?;
            }
            for prefix in &resp.common_prefixes {
                w.create_element("CommonPrefixes")
                    .write_inner_content(|w| {
                        w.create_element("Prefix")
                            .write_text_content(BytesText::new(prefix))?;
                        Ok(())
                    })?;
            }
            Ok(())
        })
        .unwrap();
    let bytes = writer.into_inner().into_inner();
    format!("{}{}", xml_header(), String::from_utf8(bytes).unwrap())
}

fn write_object_xml(
    w: &mut Writer<Cursor<Vec<u8>>>,
    obj: &ObjectMeta,
) -> std::io::Result<()> {
    w.create_element("Contents")
        .write_inner_content(|w| {
            w.create_element("Key")
                .write_text_content(BytesText::new(&obj.key))?;
            w.create_element("LastModified")
                .write_text_content(BytesText::new(&obj.last_modified.to_rfc3339()))?;
            w.create_element("ETag")
                .write_text_content(BytesText::new(&format!("\"{}\"", obj.etag)))?;
            w.create_element("Size")
                .write_text_content(BytesText::new(&obj.size.to_string()))?;
            w.create_element("StorageClass")
                .write_text_content(BytesText::new("STANDARD"))?;
            Ok(())
        })?;
    Ok(())
}

pub fn initiate_multipart_upload_xml(bucket: &str, key: &str, upload_id: &str) -> String {
    let mut writer = Writer::new(Cursor::new(Vec::new()));
    writer
        .create_element("InitiateMultipartUploadResult")
        .with_attribute(("xmlns", S3_XMLNS))
        .write_inner_content(|w| {
            w.create_element("Bucket")
                .write_text_content(BytesText::new(bucket))?;
            w.create_element("Key")
                .write_text_content(BytesText::new(key))?;
            w.create_element("UploadId")
                .write_text_content(BytesText::new(upload_id))?;
            Ok(())
        })
        .unwrap();
    let bytes = writer.into_inner().into_inner();
    format!("{}{}", xml_header(), String::from_utf8(bytes).unwrap())
}

pub fn complete_multipart_upload_xml(
    bucket: &str,
    key: &str,
    etag: &str,
    location: &str,
) -> String {
    let mut writer = Writer::new(Cursor::new(Vec::new()));
    writer
        .create_element("CompleteMultipartUploadResult")
        .with_attribute(("xmlns", S3_XMLNS))
        .write_inner_content(|w| {
            w.create_element("Location")
                .write_text_content(BytesText::new(location))?;
            w.create_element("Bucket")
                .write_text_content(BytesText::new(bucket))?;
            w.create_element("Key")
                .write_text_content(BytesText::new(key))?;
            w.create_element("ETag")
                .write_text_content(BytesText::new(&format!("\"{}\"", etag)))?;
            Ok(())
        })
        .unwrap();
    let bytes = writer.into_inner().into_inner();
    format!("{}{}", xml_header(), String::from_utf8(bytes).unwrap())
}

pub fn list_parts_xml(upload: &MultipartUpload) -> String {
    let mut writer = Writer::new(Cursor::new(Vec::new()));
    writer
        .create_element("ListPartsResult")
        .with_attribute(("xmlns", S3_XMLNS))
        .write_inner_content(|w| {
            w.create_element("Bucket")
                .write_text_content(BytesText::new(&upload.bucket))?;
            w.create_element("Key")
                .write_text_content(BytesText::new(&upload.key))?;
            w.create_element("UploadId")
                .write_text_content(BytesText::new(&upload.upload_id))?;
            for part in &upload.parts {
                write_part_xml(w, part)?;
            }
            Ok(())
        })
        .unwrap();
    let bytes = writer.into_inner().into_inner();
    format!("{}{}", xml_header(), String::from_utf8(bytes).unwrap())
}

fn write_part_xml(
    w: &mut Writer<Cursor<Vec<u8>>>,
    part: &PartInfo,
) -> std::io::Result<()> {
    w.create_element("Part")
        .write_inner_content(|w| {
            w.create_element("PartNumber")
                .write_text_content(BytesText::new(&part.part_number.to_string()))?;
            w.create_element("ETag")
                .write_text_content(BytesText::new(&format!("\"{}\"", part.etag)))?;
            w.create_element("Size")
                .write_text_content(BytesText::new(&part.size.to_string()))?;
            w.create_element("LastModified")
                .write_text_content(BytesText::new(&part.last_modified.to_rfc3339()))?;
            Ok(())
        })?;
    Ok(())
}

pub fn get_tagging_xml(tags: &HashMap<String, String>) -> String {
    let mut writer = Writer::new(Cursor::new(Vec::new()));
    writer
        .create_element("Tagging")
        .with_attribute(("xmlns", S3_XMLNS))
        .write_inner_content(|w| {
            w.create_element("TagSet")
                .write_inner_content(|w| {
                    for (k, v) in tags {
                        w.create_element("Tag")
                            .write_inner_content(|w| {
                                w.create_element("Key")
                                    .write_text_content(BytesText::new(k))?;
                                w.create_element("Value")
                                    .write_text_content(BytesText::new(v))?;
                                Ok(())
                            })?;
                    }
                    Ok(())
                })?;
            Ok(())
        })
        .unwrap();
    let bytes = writer.into_inner().into_inner();
    format!("{}{}", xml_header(), String::from_utf8(bytes).unwrap())
}

pub fn copy_object_result_xml(etag: &str, last_modified: &chrono::DateTime<chrono::Utc>) -> String {
    let mut writer = Writer::new(Cursor::new(Vec::new()));
    writer
        .create_element("CopyObjectResult")
        .with_attribute(("xmlns", S3_XMLNS))
        .write_inner_content(|w| {
            w.create_element("ETag")
                .write_text_content(BytesText::new(&format!("\"{}\"", etag)))?;
            w.create_element("LastModified")
                .write_text_content(BytesText::new(&last_modified.to_rfc3339()))?;
            Ok(())
        })
        .unwrap();
    let bytes = writer.into_inner().into_inner();
    format!("{}{}", xml_header(), String::from_utf8(bytes).unwrap())
}

pub fn delete_objects_result_xml(
    deleted: &[String],
    errors: &[(String, String, String)],
    quiet: bool,
) -> String {
    let mut writer = Writer::new(Cursor::new(Vec::new()));
    writer
        .create_element("DeleteResult")
        .with_attribute(("xmlns", S3_XMLNS))
        .write_inner_content(|w| {
            if !quiet {
                for key in deleted {
                    w.create_element("Deleted")
                        .write_inner_content(|w| {
                            w.create_element("Key")
                                .write_text_content(BytesText::new(key))?;
                            Ok(())
                        })?;
                }
            }
            for (key, code, message) in errors {
                w.create_element("Error")
                    .write_inner_content(|w| {
                        w.create_element("Key")
                            .write_text_content(BytesText::new(key))?;
                        w.create_element("Code")
                            .write_text_content(BytesText::new(code))?;
                        w.create_element("Message")
                            .write_text_content(BytesText::new(message))?;
                        Ok(())
                    })?;
            }
            Ok(())
        })
        .unwrap();
    let bytes = writer.into_inner().into_inner();
    format!("{}{}", xml_header(), String::from_utf8(bytes).unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_list_buckets_xml() {
        let buckets = vec![BucketMeta {
            name: "test-bucket".into(),
            creation_date: Utc::now(),
            anonymous_read: false,
        }];
        let xml = list_buckets_xml("owner", &buckets);
        assert!(xml.contains("xmlns=\"http://s3.amazonaws.com/doc/2006-03-01/\""));
        assert!(xml.contains("<Name>test-bucket</Name>"));
        assert!(xml.contains("<ListAllMyBucketsResult"));
    }

    #[test]
    fn test_list_objects_v2_xml() {
        let resp = ListObjectsV2Response {
            name: "mybucket".into(),
            prefix: "".into(),
            delimiter: "/".into(),
            max_keys: 1000,
            is_truncated: false,
            contents: vec![ObjectMeta {
                bucket: "mybucket".into(),
                key: "file.txt".into(),
                size: 100,
                etag: "abc123".into(),
                content_type: "text/plain".into(),
                last_modified: Utc::now(),
            }],
            common_prefixes: vec!["photos/".into()],
            next_continuation_token: None,
            key_count: 1,
        };
        let xml = list_objects_v2_xml(&resp);
        assert!(xml.contains("<ListBucketResult"));
        assert!(xml.contains("<Key>file.txt</Key>"));
        assert!(xml.contains("<Prefix>photos/</Prefix>"));
        assert!(xml.contains("<Delimiter>/</Delimiter>"));
    }

    #[test]
    fn test_error_xml() {
        let err = crate::S3Error::NoSuchKey;
        let xml = err.to_xml();
        assert!(xml.contains("<Code>NoSuchKey</Code>"));
        assert!(xml.contains("<Message>"));
    }

    #[test]
    fn test_get_tagging_xml() {
        let mut tags = HashMap::new();
        tags.insert("env".into(), "prod".into());
        let xml = get_tagging_xml(&tags);
        assert!(xml.contains("<Tagging"));
        assert!(xml.contains("<TagSet>"));
        assert!(xml.contains("<Key>env</Key>"));
        assert!(xml.contains("<Value>prod</Value>"));
    }

    #[test]
    fn test_copy_object_result_xml() {
        let xml = copy_object_result_xml("abc123", &Utc::now());
        assert!(xml.contains("<CopyObjectResult"));
        assert!(xml.contains("<ETag>"));
        assert!(xml.contains("abc123"));
        assert!(xml.contains("<LastModified>"));
    }

    #[test]
    fn test_delete_objects_result_xml() {
        let deleted = vec!["key1".to_string(), "key2".to_string()];
        let errors: Vec<(String, String, String)> = vec![];
        let xml = delete_objects_result_xml(&deleted, &errors, false);
        assert!(xml.contains("<DeleteResult"));
        assert!(xml.contains("<Deleted>"));
        assert!(xml.contains("<Key>key1</Key>"));
        assert!(xml.contains("<Key>key2</Key>"));
    }

    #[test]
    fn test_delete_objects_result_quiet() {
        let deleted = vec!["key1".to_string()];
        let errors: Vec<(String, String, String)> = vec![];
        let xml = delete_objects_result_xml(&deleted, &errors, true);
        assert!(xml.contains("<DeleteResult"));
        assert!(!xml.contains("<Deleted>"));
    }

    #[test]
    fn test_multipart_xml_responses() {
        let xml = initiate_multipart_upload_xml("mybucket", "mykey", "upload-123");
        assert!(xml.contains("<UploadId>upload-123</UploadId>"));
        assert!(xml.contains("<Bucket>mybucket</Bucket>"));

        let xml = complete_multipart_upload_xml("mybucket", "mykey", "etag123", "http://localhost/mybucket/mykey");
        assert!(xml.contains("etag123"));
    }
}
