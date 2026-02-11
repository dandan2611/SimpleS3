use quick_xml::Writer;
use quick_xml::events::BytesText;
use std::collections::HashMap;
use std::io::Cursor;

use crate::s3::types::{
    BucketMeta, CorsConfiguration, CorsRule, LifecycleConfiguration, LifecycleRule,
    LifecycleStatus, LifecycleTagFilter, ListObjectsV2Response, MultipartUpload, ObjectMeta,
    PartInfo,
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

pub fn get_object_acl_xml(public: bool) -> String {
    let mut writer = Writer::new(Cursor::new(Vec::new()));
    writer
        .create_element("AccessControlPolicy")
        .with_attribute(("xmlns", S3_XMLNS))
        .write_inner_content(|w| {
            w.create_element("Owner")
                .write_inner_content(|w| {
                    w.create_element("ID")
                        .write_text_content(BytesText::new("simples3"))?;
                    w.create_element("DisplayName")
                        .write_text_content(BytesText::new("simples3"))?;
                    Ok(())
                })?;
            w.create_element("AccessControlList")
                .write_inner_content(|w| {
                    // Owner always has FULL_CONTROL
                    write_acl_grant_canonical(w, "simples3", "simples3", "FULL_CONTROL")?;
                    if public {
                        write_acl_grant_group(
                            w,
                            "http://acs.amazonaws.com/groups/global/AllUsers",
                            "READ",
                        )?;
                    }
                    Ok(())
                })?;
            Ok(())
        })
        .unwrap();
    let bytes = writer.into_inner().into_inner();
    format!("{}{}", xml_header(), String::from_utf8(bytes).unwrap())
}

fn write_acl_grant_canonical(
    w: &mut Writer<Cursor<Vec<u8>>>,
    id: &str,
    display_name: &str,
    permission: &str,
) -> std::io::Result<()> {
    w.create_element("Grant")
        .write_inner_content(|w| {
            w.create_element("Grantee")
                .with_attribute(("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance"))
                .with_attribute(("xsi:type", "CanonicalUser"))
                .write_inner_content(|w| {
                    w.create_element("ID")
                        .write_text_content(BytesText::new(id))?;
                    w.create_element("DisplayName")
                        .write_text_content(BytesText::new(display_name))?;
                    Ok(())
                })?;
            w.create_element("Permission")
                .write_text_content(BytesText::new(permission))?;
            Ok(())
        })?;
    Ok(())
}

pub fn lifecycle_configuration_xml(config: &LifecycleConfiguration) -> String {
    let mut writer = Writer::new(Cursor::new(Vec::new()));
    writer
        .create_element("LifecycleConfiguration")
        .with_attribute(("xmlns", S3_XMLNS))
        .write_inner_content(|w| {
            for rule in &config.rules {
                w.create_element("Rule")
                    .write_inner_content(|w| {
                        w.create_element("ID")
                            .write_text_content(BytesText::new(&rule.id))?;
                        // Filter: use <And> wrapper when both prefix is non-empty and tags are present
                        let has_prefix = !rule.prefix.is_empty();
                        let has_tags = !rule.tags.is_empty();
                        let need_and = (has_prefix && has_tags) || rule.tags.len() > 1;
                        w.create_element("Filter")
                            .write_inner_content(|w| {
                                if need_and {
                                    w.create_element("And")
                                        .write_inner_content(|w| {
                                            if has_prefix {
                                                w.create_element("Prefix")
                                                    .write_text_content(BytesText::new(&rule.prefix))?;
                                            }
                                            for tag in &rule.tags {
                                                write_lifecycle_tag_xml(w, tag)?;
                                            }
                                            Ok(())
                                        })?;
                                } else if has_tags {
                                    // Single tag, no prefix
                                    write_lifecycle_tag_xml(w, &rule.tags[0])?;
                                } else {
                                    w.create_element("Prefix")
                                        .write_text_content(BytesText::new(&rule.prefix))?;
                                }
                                Ok(())
                            })?;
                        let status_str = match rule.status {
                            LifecycleStatus::Enabled => "Enabled",
                            LifecycleStatus::Disabled => "Disabled",
                        };
                        w.create_element("Status")
                            .write_text_content(BytesText::new(status_str))?;
                        w.create_element("Expiration")
                            .write_inner_content(|w| {
                                if let Some(ref date) = rule.expiration_date {
                                    w.create_element("Date")
                                        .write_text_content(BytesText::new(date))?;
                                } else {
                                    w.create_element("Days")
                                        .write_text_content(BytesText::new(
                                            &rule.expiration_days.to_string(),
                                        ))?;
                                }
                                Ok(())
                            })?;
                        Ok(())
                    })?;
            }
            Ok(())
        })
        .unwrap();
    let bytes = writer.into_inner().into_inner();
    format!("{}{}", xml_header(), String::from_utf8(bytes).unwrap())
}

fn write_lifecycle_tag_xml(
    w: &mut Writer<Cursor<Vec<u8>>>,
    tag: &LifecycleTagFilter,
) -> std::io::Result<()> {
    w.create_element("Tag")
        .write_inner_content(|w| {
            w.create_element("Key")
                .write_text_content(BytesText::new(&tag.key))?;
            w.create_element("Value")
                .write_text_content(BytesText::new(&tag.value))?;
            Ok(())
        })?;
    Ok(())
}

pub fn parse_lifecycle_configuration_xml(
    data: &[u8],
) -> Result<LifecycleConfiguration, crate::S3Error> {
    use quick_xml::Reader;
    use quick_xml::events::Event;

    let mut reader = Reader::from_reader(data);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut rules = Vec::new();

    // Per-rule state
    let mut in_rule = false;
    let mut in_id = false;
    let mut in_filter = false;
    let mut in_and = false;
    let mut in_prefix = false;
    let mut in_status = false;
    let mut in_expiration = false;
    let mut in_days = false;
    let mut in_date = false;
    let mut in_tag = false;
    let mut in_tag_key = false;
    let mut in_tag_value = false;

    let mut current_id = String::new();
    let mut current_prefix = String::new();
    let mut current_status = String::new();
    let mut current_days = String::new();
    let mut current_date = String::new();
    let mut current_tags: Vec<LifecycleTagFilter> = Vec::new();
    let mut current_tag_key = String::new();
    let mut current_tag_value = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => match e.name().as_ref() {
                b"Rule" => {
                    in_rule = true;
                    current_id.clear();
                    current_prefix.clear();
                    current_status.clear();
                    current_days.clear();
                    current_date.clear();
                    current_tags.clear();
                }
                b"ID" if in_rule => in_id = true,
                b"Filter" if in_rule => in_filter = true,
                b"And" if in_filter => in_and = true,
                b"Prefix" if in_filter || in_and => in_prefix = true,
                b"Tag" if in_filter || in_and => {
                    in_tag = true;
                    current_tag_key.clear();
                    current_tag_value.clear();
                }
                b"Key" if in_tag => in_tag_key = true,
                b"Value" if in_tag => in_tag_value = true,
                b"Status" if in_rule => in_status = true,
                b"Expiration" if in_rule => in_expiration = true,
                b"Days" if in_expiration => in_days = true,
                b"Date" if in_expiration => in_date = true,
                _ => {}
            },
            Ok(Event::Text(e)) => {
                let text = e
                    .unescape()
                    .map_err(|e| crate::S3Error::InvalidArgument(e.to_string()))?
                    .into_owned();
                if in_tag_key {
                    current_tag_key = text;
                } else if in_tag_value {
                    current_tag_value = text;
                } else if in_id {
                    current_id = text;
                } else if in_prefix {
                    current_prefix = text;
                } else if in_status {
                    current_status = text;
                } else if in_days {
                    current_days = text;
                } else if in_date {
                    current_date = text;
                }
            }
            Ok(Event::End(e)) => match e.name().as_ref() {
                b"Rule" => {
                    let status = match current_status.as_str() {
                        "Enabled" => LifecycleStatus::Enabled,
                        "Disabled" => LifecycleStatus::Disabled,
                        other => {
                            return Err(crate::S3Error::InvalidArgument(format!(
                                "Invalid lifecycle status: {}",
                                other
                            )));
                        }
                    };
                    let has_days = !current_days.is_empty();
                    let has_date = !current_date.is_empty();
                    if has_days && has_date {
                        return Err(crate::S3Error::InvalidArgument(
                            "Expiration must specify either Days or Date, not both".to_string(),
                        ));
                    }
                    let (days, date) = if has_date {
                        // Validate date parses as ISO 8601
                        chrono::DateTime::parse_from_rfc3339(&current_date).map_err(|_| {
                            crate::S3Error::InvalidArgument(
                                "Invalid expiration date format (expected ISO 8601)".to_string(),
                            )
                        })?;
                        (0, Some(current_date.clone()))
                    } else {
                        let d: u32 = current_days.parse().map_err(|_| {
                            crate::S3Error::InvalidArgument(
                                "Invalid expiration days".to_string(),
                            )
                        })?;
                        if d == 0 {
                            return Err(crate::S3Error::InvalidArgument(
                                "Expiration days must be greater than 0".to_string(),
                            ));
                        }
                        (d, None)
                    };
                    rules.push(LifecycleRule {
                        id: current_id.clone(),
                        prefix: current_prefix.clone(),
                        status,
                        expiration_days: days,
                        expiration_date: date,
                        tags: current_tags.clone(),
                    });
                    in_rule = false;
                }
                b"ID" => in_id = false,
                b"Filter" => in_filter = false,
                b"And" => in_and = false,
                b"Prefix" if in_prefix => in_prefix = false,
                b"Tag" if in_tag => {
                    current_tags.push(LifecycleTagFilter {
                        key: current_tag_key.clone(),
                        value: current_tag_value.clone(),
                    });
                    in_tag = false;
                }
                b"Key" if in_tag => in_tag_key = false,
                b"Value" if in_tag => in_tag_value = false,
                b"Status" => in_status = false,
                b"Expiration" => in_expiration = false,
                b"Days" => in_days = false,
                b"Date" => in_date = false,
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(crate::S3Error::InvalidArgument(e.to_string()));
            }
            _ => {}
        }
        buf.clear();
    }

    Ok(LifecycleConfiguration { rules })
}

pub fn cors_configuration_xml(config: &CorsConfiguration) -> String {
    let mut writer = Writer::new(Cursor::new(Vec::new()));
    writer
        .create_element("CORSConfiguration")
        .with_attribute(("xmlns", S3_XMLNS))
        .write_inner_content(|w| {
            for rule in &config.rules {
                w.create_element("CORSRule")
                    .write_inner_content(|w| {
                        if let Some(ref id) = rule.id {
                            w.create_element("ID")
                                .write_text_content(BytesText::new(id))?;
                        }
                        for origin in &rule.allowed_origins {
                            w.create_element("AllowedOrigin")
                                .write_text_content(BytesText::new(origin))?;
                        }
                        for method in &rule.allowed_methods {
                            w.create_element("AllowedMethod")
                                .write_text_content(BytesText::new(method))?;
                        }
                        for header in &rule.allowed_headers {
                            w.create_element("AllowedHeader")
                                .write_text_content(BytesText::new(header))?;
                        }
                        for header in &rule.expose_headers {
                            w.create_element("ExposeHeader")
                                .write_text_content(BytesText::new(header))?;
                        }
                        if let Some(max_age) = rule.max_age_seconds {
                            w.create_element("MaxAgeSeconds")
                                .write_text_content(BytesText::new(&max_age.to_string()))?;
                        }
                        Ok(())
                    })?;
            }
            Ok(())
        })
        .unwrap();
    let bytes = writer.into_inner().into_inner();
    format!("{}{}", xml_header(), String::from_utf8(bytes).unwrap())
}

pub fn parse_cors_configuration_xml(
    data: &[u8],
) -> Result<CorsConfiguration, crate::S3Error> {
    use quick_xml::Reader;
    use quick_xml::events::Event;

    let mut reader = Reader::from_reader(data);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut rules = Vec::new();

    let mut in_rule = false;
    let mut in_id = false;
    let mut in_allowed_origin = false;
    let mut in_allowed_method = false;
    let mut in_allowed_header = false;
    let mut in_expose_header = false;
    let mut in_max_age = false;

    let mut current_id: Option<String> = None;
    let mut current_origins: Vec<String> = Vec::new();
    let mut current_methods: Vec<String> = Vec::new();
    let mut current_headers: Vec<String> = Vec::new();
    let mut current_expose: Vec<String> = Vec::new();
    let mut current_max_age: Option<u32> = None;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => match e.name().as_ref() {
                b"CORSRule" => {
                    in_rule = true;
                    current_id = None;
                    current_origins.clear();
                    current_methods.clear();
                    current_headers.clear();
                    current_expose.clear();
                    current_max_age = None;
                }
                b"ID" if in_rule => in_id = true,
                b"AllowedOrigin" if in_rule => in_allowed_origin = true,
                b"AllowedMethod" if in_rule => in_allowed_method = true,
                b"AllowedHeader" if in_rule => in_allowed_header = true,
                b"ExposeHeader" if in_rule => in_expose_header = true,
                b"MaxAgeSeconds" if in_rule => in_max_age = true,
                _ => {}
            },
            Ok(Event::Text(e)) => {
                let text = e
                    .unescape()
                    .map_err(|e| crate::S3Error::InvalidArgument(e.to_string()))?
                    .into_owned();
                if in_id {
                    current_id = Some(text);
                } else if in_allowed_origin {
                    current_origins.push(text);
                } else if in_allowed_method {
                    current_methods.push(text);
                } else if in_allowed_header {
                    current_headers.push(text);
                } else if in_expose_header {
                    current_expose.push(text);
                } else if in_max_age {
                    current_max_age = text.parse().ok();
                }
            }
            Ok(Event::End(e)) => match e.name().as_ref() {
                b"CORSRule" => {
                    if current_origins.is_empty() {
                        return Err(crate::S3Error::InvalidArgument(
                            "CORSRule must have at least one AllowedOrigin".to_string(),
                        ));
                    }
                    if current_methods.is_empty() {
                        return Err(crate::S3Error::InvalidArgument(
                            "CORSRule must have at least one AllowedMethod".to_string(),
                        ));
                    }
                    rules.push(CorsRule {
                        id: current_id.clone(),
                        allowed_origins: current_origins.clone(),
                        allowed_methods: current_methods.clone(),
                        allowed_headers: current_headers.clone(),
                        expose_headers: current_expose.clone(),
                        max_age_seconds: current_max_age,
                    });
                    in_rule = false;
                }
                b"ID" => in_id = false,
                b"AllowedOrigin" => in_allowed_origin = false,
                b"AllowedMethod" => in_allowed_method = false,
                b"AllowedHeader" => in_allowed_header = false,
                b"ExposeHeader" => in_expose_header = false,
                b"MaxAgeSeconds" => in_max_age = false,
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(crate::S3Error::InvalidArgument(e.to_string()));
            }
            _ => {}
        }
        buf.clear();
    }

    Ok(CorsConfiguration { rules })
}

fn write_acl_grant_group(
    w: &mut Writer<Cursor<Vec<u8>>>,
    uri: &str,
    permission: &str,
) -> std::io::Result<()> {
    w.create_element("Grant")
        .write_inner_content(|w| {
            w.create_element("Grantee")
                .with_attribute(("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance"))
                .with_attribute(("xsi:type", "Group"))
                .write_inner_content(|w| {
                    w.create_element("URI")
                        .write_text_content(BytesText::new(uri))?;
                    Ok(())
                })?;
            w.create_element("Permission")
                .write_text_content(BytesText::new(permission))?;
            Ok(())
        })?;
    Ok(())
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
            anonymous_list_public: false,
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
                public: false,
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

    #[test]
    fn test_get_object_acl_xml_private() {
        let xml = get_object_acl_xml(false);
        assert!(xml.contains("<AccessControlPolicy"));
        assert!(xml.contains("<Permission>FULL_CONTROL</Permission>"));
        assert!(!xml.contains("AllUsers"));
    }

    #[test]
    fn test_lifecycle_xml_roundtrip() {
        use crate::s3::types::{LifecycleConfiguration, LifecycleRule, LifecycleStatus};
        let config = LifecycleConfiguration {
            rules: vec![
                LifecycleRule {
                    id: "expire-logs".into(),
                    prefix: "logs/".into(),
                    status: LifecycleStatus::Enabled,
                    expiration_days: 30,
                    expiration_date: None,
                    tags: vec![],
                },
                LifecycleRule {
                    id: "expire-tmp".into(),
                    prefix: "tmp/".into(),
                    status: LifecycleStatus::Disabled,
                    expiration_days: 7,
                    expiration_date: None,
                    tags: vec![],
                },
            ],
        };
        let xml = lifecycle_configuration_xml(&config);
        assert!(xml.contains("<LifecycleConfiguration"));
        assert!(xml.contains("<ID>expire-logs</ID>"));
        assert!(xml.contains("<Prefix>logs/</Prefix>"));
        assert!(xml.contains("<Status>Enabled</Status>"));
        assert!(xml.contains("<Days>30</Days>"));

        let parsed = parse_lifecycle_configuration_xml(xml.as_bytes()).unwrap();
        assert_eq!(parsed.rules.len(), 2);
        assert_eq!(parsed.rules[0].id, "expire-logs");
        assert_eq!(parsed.rules[0].prefix, "logs/");
        assert_eq!(parsed.rules[0].status, LifecycleStatus::Enabled);
        assert_eq!(parsed.rules[0].expiration_days, 30);
        assert!(parsed.rules[0].expiration_date.is_none());
        assert!(parsed.rules[0].tags.is_empty());
        assert_eq!(parsed.rules[1].status, LifecycleStatus::Disabled);
    }

    #[test]
    fn test_lifecycle_xml_invalid_days() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?><LifecycleConfiguration><Rule><ID>r</ID><Filter><Prefix></Prefix></Filter><Status>Enabled</Status><Expiration><Days>0</Days></Expiration></Rule></LifecycleConfiguration>"#;
        let result = parse_lifecycle_configuration_xml(xml.as_bytes());
        assert!(result.is_err());
    }

    #[test]
    fn test_lifecycle_xml_tag_filter_roundtrip() {
        use crate::s3::types::{LifecycleConfiguration, LifecycleRule, LifecycleStatus, LifecycleTagFilter};
        let config = LifecycleConfiguration {
            rules: vec![LifecycleRule {
                id: "tag-rule".into(),
                prefix: String::new(),
                status: LifecycleStatus::Enabled,
                expiration_days: 10,
                expiration_date: None,
                tags: vec![LifecycleTagFilter {
                    key: "env".into(),
                    value: "test".into(),
                }],
            }],
        };
        let xml = lifecycle_configuration_xml(&config);
        // Single tag without prefix: no <And> wrapper
        assert!(!xml.contains("<And>"));
        assert!(xml.contains("<Tag>"));
        assert!(xml.contains("<Key>env</Key>"));
        assert!(xml.contains("<Value>test</Value>"));

        let parsed = parse_lifecycle_configuration_xml(xml.as_bytes()).unwrap();
        assert_eq!(parsed.rules[0].tags.len(), 1);
        assert_eq!(parsed.rules[0].tags[0].key, "env");
        assert_eq!(parsed.rules[0].tags[0].value, "test");
        assert!(parsed.rules[0].prefix.is_empty());
    }

    #[test]
    fn test_lifecycle_xml_and_filter_roundtrip() {
        use crate::s3::types::{LifecycleConfiguration, LifecycleRule, LifecycleStatus, LifecycleTagFilter};
        let config = LifecycleConfiguration {
            rules: vec![LifecycleRule {
                id: "and-rule".into(),
                prefix: "logs/".into(),
                status: LifecycleStatus::Enabled,
                expiration_days: 5,
                expiration_date: None,
                tags: vec![
                    LifecycleTagFilter { key: "env".into(), value: "staging".into() },
                    LifecycleTagFilter { key: "team".into(), value: "infra".into() },
                ],
            }],
        };
        let xml = lifecycle_configuration_xml(&config);
        assert!(xml.contains("<And>"));
        assert!(xml.contains("<Prefix>logs/</Prefix>"));
        assert!(xml.contains("<Key>env</Key>"));
        assert!(xml.contains("<Key>team</Key>"));

        let parsed = parse_lifecycle_configuration_xml(xml.as_bytes()).unwrap();
        assert_eq!(parsed.rules[0].prefix, "logs/");
        assert_eq!(parsed.rules[0].tags.len(), 2);
        assert_eq!(parsed.rules[0].tags[0].key, "env");
        assert_eq!(parsed.rules[0].tags[1].key, "team");
    }

    #[test]
    fn test_lifecycle_xml_date_expiration_roundtrip() {
        use crate::s3::types::{LifecycleConfiguration, LifecycleRule, LifecycleStatus};
        let config = LifecycleConfiguration {
            rules: vec![LifecycleRule {
                id: "date-rule".into(),
                prefix: "archive/".into(),
                status: LifecycleStatus::Enabled,
                expiration_days: 0,
                expiration_date: Some("2025-12-31T00:00:00+00:00".into()),
                tags: vec![],
            }],
        };
        let xml = lifecycle_configuration_xml(&config);
        assert!(xml.contains("<Date>2025-12-31T00:00:00+00:00</Date>"));
        assert!(!xml.contains("<Days>"));

        let parsed = parse_lifecycle_configuration_xml(xml.as_bytes()).unwrap();
        assert_eq!(parsed.rules[0].expiration_days, 0);
        assert_eq!(
            parsed.rules[0].expiration_date.as_deref(),
            Some("2025-12-31T00:00:00+00:00")
        );
    }

    #[test]
    fn test_lifecycle_xml_both_days_and_date_error() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?><LifecycleConfiguration><Rule><ID>r</ID><Filter><Prefix></Prefix></Filter><Status>Enabled</Status><Expiration><Days>5</Days><Date>2025-12-31T00:00:00+00:00</Date></Expiration></Rule></LifecycleConfiguration>"#;
        let result = parse_lifecycle_configuration_xml(xml.as_bytes());
        assert!(result.is_err());
    }

    #[test]
    fn test_cors_xml_roundtrip() {
        use crate::s3::types::{CorsConfiguration, CorsRule};
        let config = CorsConfiguration {
            rules: vec![
                CorsRule {
                    id: Some("rule-1".into()),
                    allowed_origins: vec!["https://example.com".into(), "https://app.example.com".into()],
                    allowed_methods: vec!["GET".into(), "PUT".into()],
                    allowed_headers: vec!["*".into()],
                    expose_headers: vec!["x-amz-request-id".into()],
                    max_age_seconds: Some(3600),
                },
                CorsRule {
                    id: None,
                    allowed_origins: vec!["*".into()],
                    allowed_methods: vec!["GET".into()],
                    allowed_headers: vec![],
                    expose_headers: vec![],
                    max_age_seconds: None,
                },
            ],
        };
        let xml = cors_configuration_xml(&config);
        assert!(xml.contains("<CORSConfiguration"));
        assert!(xml.contains("<AllowedOrigin>https://example.com</AllowedOrigin>"));
        assert!(xml.contains("<AllowedMethod>GET</AllowedMethod>"));
        assert!(xml.contains("<MaxAgeSeconds>3600</MaxAgeSeconds>"));
        assert!(xml.contains("<ID>rule-1</ID>"));

        let parsed = parse_cors_configuration_xml(xml.as_bytes()).unwrap();
        assert_eq!(parsed.rules.len(), 2);
        assert_eq!(parsed.rules[0].id.as_deref(), Some("rule-1"));
        assert_eq!(parsed.rules[0].allowed_origins.len(), 2);
        assert_eq!(parsed.rules[0].allowed_methods.len(), 2);
        assert_eq!(parsed.rules[0].allowed_headers, vec!["*"]);
        assert_eq!(parsed.rules[0].expose_headers, vec!["x-amz-request-id"]);
        assert_eq!(parsed.rules[0].max_age_seconds, Some(3600));
        assert_eq!(parsed.rules[1].allowed_origins, vec!["*"]);
        assert!(parsed.rules[1].id.is_none());
        assert!(parsed.rules[1].max_age_seconds.is_none());
    }

    #[test]
    fn test_cors_xml_missing_origin() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?><CORSConfiguration><CORSRule><AllowedMethod>GET</AllowedMethod></CORSRule></CORSConfiguration>"#;
        let result = parse_cors_configuration_xml(xml.as_bytes());
        assert!(result.is_err());
    }

    #[test]
    fn test_cors_xml_missing_method() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?><CORSConfiguration><CORSRule><AllowedOrigin>*</AllowedOrigin></CORSRule></CORSConfiguration>"#;
        let result = parse_cors_configuration_xml(xml.as_bytes());
        assert!(result.is_err());
    }

    #[test]
    fn test_get_object_acl_xml_public() {
        let xml = get_object_acl_xml(true);
        assert!(xml.contains("<AccessControlPolicy"));
        assert!(xml.contains("<Permission>FULL_CONTROL</Permission>"));
        assert!(xml.contains("AllUsers"));
        assert!(xml.contains("<Permission>READ</Permission>"));
    }
}
