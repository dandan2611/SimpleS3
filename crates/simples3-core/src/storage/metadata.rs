use crate::error::S3Error;
use crate::s3::types::{
    AccessKeyRecord, BucketMeta, BucketPolicy, CorsConfiguration, LifecycleConfiguration,
    ListObjectsV2Request, ListObjectsV2Response, MultipartUpload, ObjectMeta, PartInfo,
};
use chrono::Utc;
use sled::Db;
use std::collections::HashMap;
use std::path::Path;

const BUCKETS_TREE: &str = "buckets";
const CREDENTIALS_TREE: &str = "credentials";
const MULTIPART_TREE: &str = "multipart";
const TAGGING_TREE: &str = "tagging";
const LIFECYCLE_TREE: &str = "lifecycle";
const POLICIES_TREE: &str = "policies";
const CORS_TREE: &str = "cors";

fn objects_tree_name(bucket: &str) -> String {
    format!("objects:{}", bucket)
}

/// Validate bucket name against S3 naming rules.
fn validate_bucket_name(name: &str) -> Result<(), S3Error> {
    if name.len() < 3 || name.len() > 63 {
        return Err(S3Error::InvalidArgument(
            "Bucket name must be between 3 and 63 characters".into(),
        ));
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '.')
    {
        return Err(S3Error::InvalidArgument(
            "Bucket name must contain only lowercase letters, numbers, hyphens, and periods".into(),
        ));
    }
    if name.starts_with('-')
        || name.starts_with('.')
        || name.ends_with('-')
        || name.ends_with('.')
    {
        return Err(S3Error::InvalidArgument(
            "Bucket name must not start or end with a hyphen or period".into(),
        ));
    }
    if name.contains("..") {
        return Err(S3Error::InvalidArgument(
            "Bucket name must not contain consecutive periods".into(),
        ));
    }
    Ok(())
}

#[derive(Clone)]
pub struct MetadataStore {
    db: Db,
}

impl MetadataStore {
    pub fn open(path: &Path) -> Result<Self, S3Error> {
        let db = sled::open(path).map_err(|e| S3Error::InternalError(e.to_string()))?;
        Ok(Self { db })
    }

    // --- Bucket operations ---

    pub fn create_bucket(&self, name: &str) -> Result<BucketMeta, S3Error> {
        validate_bucket_name(name)?;
        let tree = self.db.open_tree(BUCKETS_TREE).map_err(|e| S3Error::InternalError(e.to_string()))?;
        if tree.contains_key(name).map_err(|e| S3Error::InternalError(e.to_string()))? {
            return Err(S3Error::BucketAlreadyExists);
        }
        let meta = BucketMeta {
            name: name.to_string(),
            creation_date: Utc::now(),
            anonymous_read: false,
            anonymous_list_public: false,
        };
        let json = serde_json::to_vec(&meta).map_err(|e| S3Error::InternalError(e.to_string()))?;
        tree.insert(name, json).map_err(|e| S3Error::InternalError(e.to_string()))?;
        Ok(meta)
    }

    pub fn get_bucket(&self, name: &str) -> Result<BucketMeta, S3Error> {
        let tree = self.db.open_tree(BUCKETS_TREE).map_err(|e| S3Error::InternalError(e.to_string()))?;
        let val = tree.get(name).map_err(|e| S3Error::InternalError(e.to_string()))?;
        match val {
            Some(bytes) => {
                serde_json::from_slice(&bytes).map_err(|e| S3Error::InternalError(e.to_string()))
            }
            None => Err(S3Error::NoSuchBucket),
        }
    }

    pub fn list_buckets(&self) -> Result<Vec<BucketMeta>, S3Error> {
        let tree = self.db.open_tree(BUCKETS_TREE).map_err(|e| S3Error::InternalError(e.to_string()))?;
        let mut buckets = Vec::new();
        for item in tree.iter() {
            let (_, val) = item.map_err(|e| S3Error::InternalError(e.to_string()))?;
            let meta: BucketMeta =
                serde_json::from_slice(&val).map_err(|e| S3Error::InternalError(e.to_string()))?;
            buckets.push(meta);
        }
        Ok(buckets)
    }

    pub fn delete_bucket(&self, name: &str) -> Result<(), S3Error> {
        // Check bucket exists
        let _ = self.get_bucket(name)?;

        // Check bucket is empty
        let obj_tree_name = objects_tree_name(name);
        let obj_tree = self.db.open_tree(&obj_tree_name).map_err(|e| S3Error::InternalError(e.to_string()))?;
        if !obj_tree.is_empty() {
            return Err(S3Error::BucketNotEmpty);
        }

        let tree = self.db.open_tree(BUCKETS_TREE).map_err(|e| S3Error::InternalError(e.to_string()))?;
        tree.remove(name).map_err(|e| S3Error::InternalError(e.to_string()))?;
        self.db.drop_tree(&obj_tree_name).map_err(|e| S3Error::InternalError(e.to_string()))?;

        // Clean up lifecycle, policy, and CORS entries
        let lifecycle_tree = self.db.open_tree(LIFECYCLE_TREE).map_err(|e| S3Error::InternalError(e.to_string()))?;
        let _ = lifecycle_tree.remove(name);
        let policies_tree = self.db.open_tree(POLICIES_TREE).map_err(|e| S3Error::InternalError(e.to_string()))?;
        let _ = policies_tree.remove(name);
        let cors_tree = self.db.open_tree(CORS_TREE).map_err(|e| S3Error::InternalError(e.to_string()))?;
        let _ = cors_tree.remove(name);

        Ok(())
    }

    pub fn set_bucket_anonymous_read(&self, name: &str, anonymous: bool) -> Result<(), S3Error> {
        let mut meta = self.get_bucket(name)?;
        meta.anonymous_read = anonymous;
        let tree = self.db.open_tree(BUCKETS_TREE).map_err(|e| S3Error::InternalError(e.to_string()))?;
        let json = serde_json::to_vec(&meta).map_err(|e| S3Error::InternalError(e.to_string()))?;
        tree.insert(name, json).map_err(|e| S3Error::InternalError(e.to_string()))?;
        Ok(())
    }

    pub fn set_bucket_anonymous_list_public(&self, name: &str, enabled: bool) -> Result<(), S3Error> {
        let mut meta = self.get_bucket(name)?;
        meta.anonymous_list_public = enabled;
        let tree = self.db.open_tree(BUCKETS_TREE).map_err(|e| S3Error::InternalError(e.to_string()))?;
        let json = serde_json::to_vec(&meta).map_err(|e| S3Error::InternalError(e.to_string()))?;
        tree.insert(name, json).map_err(|e| S3Error::InternalError(e.to_string()))?;
        Ok(())
    }

    // --- Object metadata ---

    pub fn put_object_meta(&self, meta: &ObjectMeta) -> Result<(), S3Error> {
        let tree_name = objects_tree_name(&meta.bucket);
        let tree = self.db.open_tree(&tree_name).map_err(|e| S3Error::InternalError(e.to_string()))?;
        let json = serde_json::to_vec(meta).map_err(|e| S3Error::InternalError(e.to_string()))?;
        tree.insert(&meta.key, json).map_err(|e| S3Error::InternalError(e.to_string()))?;
        Ok(())
    }

    pub fn get_object_meta(&self, bucket: &str, key: &str) -> Result<ObjectMeta, S3Error> {
        let tree_name = objects_tree_name(bucket);
        let tree = self.db.open_tree(&tree_name).map_err(|e| S3Error::InternalError(e.to_string()))?;
        let val = tree.get(key).map_err(|e| S3Error::InternalError(e.to_string()))?;
        match val {
            Some(bytes) => {
                serde_json::from_slice(&bytes).map_err(|e| S3Error::InternalError(e.to_string()))
            }
            None => Err(S3Error::NoSuchKey),
        }
    }

    pub fn delete_object_meta(&self, bucket: &str, key: &str) -> Result<(), S3Error> {
        let tree_name = objects_tree_name(bucket);
        let tree = self.db.open_tree(&tree_name).map_err(|e| S3Error::InternalError(e.to_string()))?;
        tree.remove(key).map_err(|e| S3Error::InternalError(e.to_string()))?;
        // Clean up any tagging for this object
        let tag_tree = self.db.open_tree(TAGGING_TREE).map_err(|e| S3Error::InternalError(e.to_string()))?;
        let tag_key = format!("{}:{}", bucket, key);
        tag_tree.remove(tag_key.as_bytes()).map_err(|e| S3Error::InternalError(e.to_string()))?;
        Ok(())
    }

    pub fn list_objects_v2(&self, req: &ListObjectsV2Request) -> Result<ListObjectsV2Response, S3Error> {
        let tree_name = objects_tree_name(&req.bucket);
        let tree = self.db.open_tree(&tree_name).map_err(|e| S3Error::InternalError(e.to_string()))?;

        let mut all_objects: Vec<ObjectMeta> = Vec::new();
        let prefix_bytes = req.prefix.as_bytes();

        for item in tree.iter() {
            let (key_bytes, val) = item.map_err(|e| S3Error::InternalError(e.to_string()))?;
            let key_str = String::from_utf8_lossy(&key_bytes);
            if key_str.as_bytes().starts_with(prefix_bytes) {
                let meta: ObjectMeta = serde_json::from_slice(&val)
                    .map_err(|e| S3Error::InternalError(e.to_string()))?;
                all_objects.push(meta);
            }
        }

        // Sort by key
        all_objects.sort_by(|a, b| a.key.cmp(&b.key));

        // Apply start_after or continuation_token
        let start_after = req
            .continuation_token
            .as_deref()
            .or(req.start_after.as_deref());
        if let Some(start) = start_after {
            all_objects.retain(|o| o.key.as_str() > start);
        }

        // Handle delimiter grouping
        let mut contents = Vec::new();
        let mut common_prefixes = std::collections::BTreeSet::new();

        if req.delimiter.is_empty() {
            contents = all_objects;
        } else {
            for obj in &all_objects {
                let relative = &obj.key[req.prefix.len()..];
                if let Some(idx) = relative.find(&req.delimiter) {
                    let cp = format!("{}{}", &req.prefix, &relative[..=idx]);
                    common_prefixes.insert(cp);
                } else {
                    contents.push(obj.clone());
                }
            }
        }

        let common_prefixes: Vec<String> = common_prefixes.into_iter().collect();
        let total_count = contents.len() as u32 + common_prefixes.len() as u32;
        let is_truncated = total_count > req.max_keys;

        let max = req.max_keys as usize;
        let truncated_contents: Vec<ObjectMeta> = contents.into_iter().take(max).collect();
        let next_token = if is_truncated {
            truncated_contents.last().map(|o| o.key.clone())
        } else {
            None
        };

        let key_count = truncated_contents.len() as u32;

        Ok(ListObjectsV2Response {
            name: req.bucket.clone(),
            prefix: req.prefix.clone(),
            delimiter: req.delimiter.clone(),
            max_keys: req.max_keys,
            is_truncated,
            contents: truncated_contents,
            common_prefixes,
            next_continuation_token: next_token,
            key_count,
        })
    }

    // --- Tagging operations ---

    pub fn put_object_tagging(&self, bucket: &str, key: &str, tags: &HashMap<String, String>) -> Result<(), S3Error> {
        // Verify object exists
        let _ = self.get_object_meta(bucket, key)?;
        let tree = self.db.open_tree(TAGGING_TREE).map_err(|e| S3Error::InternalError(e.to_string()))?;
        let tag_key = format!("{}:{}", bucket, key);
        let json = serde_json::to_vec(tags).map_err(|e| S3Error::InternalError(e.to_string()))?;
        tree.insert(tag_key.as_bytes(), json).map_err(|e| S3Error::InternalError(e.to_string()))?;
        Ok(())
    }

    pub fn get_object_tagging(&self, bucket: &str, key: &str) -> Result<HashMap<String, String>, S3Error> {
        // Verify object exists
        let _ = self.get_object_meta(bucket, key)?;
        let tree = self.db.open_tree(TAGGING_TREE).map_err(|e| S3Error::InternalError(e.to_string()))?;
        let tag_key = format!("{}:{}", bucket, key);
        match tree.get(tag_key.as_bytes()).map_err(|e| S3Error::InternalError(e.to_string()))? {
            Some(bytes) => serde_json::from_slice(&bytes).map_err(|e| S3Error::InternalError(e.to_string())),
            None => Ok(HashMap::new()),
        }
    }

    pub fn delete_object_tagging(&self, bucket: &str, key: &str) -> Result<(), S3Error> {
        // Verify object exists
        let _ = self.get_object_meta(bucket, key)?;
        let tree = self.db.open_tree(TAGGING_TREE).map_err(|e| S3Error::InternalError(e.to_string()))?;
        let tag_key = format!("{}:{}", bucket, key);
        tree.remove(tag_key.as_bytes()).map_err(|e| S3Error::InternalError(e.to_string()))?;
        Ok(())
    }

    // --- Credential operations ---

    pub fn create_credential(&self, access_key_id: &str, secret_access_key: &str, description: &str) -> Result<AccessKeyRecord, S3Error> {
        let tree = self.db.open_tree(CREDENTIALS_TREE).map_err(|e| S3Error::InternalError(e.to_string()))?;
        if tree.contains_key(access_key_id).map_err(|e| S3Error::InternalError(e.to_string()))? {
            return Err(S3Error::InvalidArgument("Credential already exists".into()));
        }
        let record = AccessKeyRecord {
            access_key_id: access_key_id.to_string(),
            secret_access_key: secret_access_key.to_string(),
            description: description.to_string(),
            created: Utc::now(),
            active: true,
        };
        let json = serde_json::to_vec(&record).map_err(|e| S3Error::InternalError(e.to_string()))?;
        tree.insert(access_key_id, json).map_err(|e| S3Error::InternalError(e.to_string()))?;
        Ok(record)
    }

    pub fn get_credential(&self, access_key_id: &str) -> Result<AccessKeyRecord, S3Error> {
        let tree = self.db.open_tree(CREDENTIALS_TREE).map_err(|e| S3Error::InternalError(e.to_string()))?;
        let val = tree.get(access_key_id).map_err(|e| S3Error::InternalError(e.to_string()))?;
        match val {
            Some(bytes) => {
                serde_json::from_slice(&bytes).map_err(|e| S3Error::InternalError(e.to_string()))
            }
            None => Err(S3Error::AccessDenied),
        }
    }

    pub fn list_credentials(&self) -> Result<Vec<AccessKeyRecord>, S3Error> {
        let tree = self.db.open_tree(CREDENTIALS_TREE).map_err(|e| S3Error::InternalError(e.to_string()))?;
        let mut creds = Vec::new();
        for item in tree.iter() {
            let (_, val) = item.map_err(|e| S3Error::InternalError(e.to_string()))?;
            let record: AccessKeyRecord =
                serde_json::from_slice(&val).map_err(|e| S3Error::InternalError(e.to_string()))?;
            creds.push(record);
        }
        Ok(creds)
    }

    pub fn revoke_credential(&self, access_key_id: &str) -> Result<(), S3Error> {
        let tree = self.db.open_tree(CREDENTIALS_TREE).map_err(|e| S3Error::InternalError(e.to_string()))?;
        let val = tree.get(access_key_id).map_err(|e| S3Error::InternalError(e.to_string()))?;
        match val {
            Some(bytes) => {
                let mut record: AccessKeyRecord =
                    serde_json::from_slice(&bytes).map_err(|e| S3Error::InternalError(e.to_string()))?;
                record.active = false;
                let json = serde_json::to_vec(&record).map_err(|e| S3Error::InternalError(e.to_string()))?;
                tree.insert(access_key_id, json).map_err(|e| S3Error::InternalError(e.to_string()))?;
                Ok(())
            }
            None => Err(S3Error::AccessDenied),
        }
    }

    pub fn delete_credential(&self, access_key_id: &str) -> Result<(), S3Error> {
        let tree = self.db.open_tree(CREDENTIALS_TREE).map_err(|e| S3Error::InternalError(e.to_string()))?;
        tree.remove(access_key_id).map_err(|e| S3Error::InternalError(e.to_string()))?;
        Ok(())
    }

    // --- Multipart operations ---

    pub fn create_multipart_upload(&self, upload: &MultipartUpload) -> Result<(), S3Error> {
        let tree = self.db.open_tree(MULTIPART_TREE).map_err(|e| S3Error::InternalError(e.to_string()))?;
        let json = serde_json::to_vec(upload).map_err(|e| S3Error::InternalError(e.to_string()))?;
        tree.insert(&upload.upload_id, json).map_err(|e| S3Error::InternalError(e.to_string()))?;
        Ok(())
    }

    pub fn get_multipart_upload(&self, upload_id: &str) -> Result<MultipartUpload, S3Error> {
        let tree = self.db.open_tree(MULTIPART_TREE).map_err(|e| S3Error::InternalError(e.to_string()))?;
        let val = tree.get(upload_id).map_err(|e| S3Error::InternalError(e.to_string()))?;
        match val {
            Some(bytes) => {
                serde_json::from_slice(&bytes).map_err(|e| S3Error::InternalError(e.to_string()))
            }
            None => Err(S3Error::NoSuchUpload),
        }
    }

    pub fn add_part_to_upload(&self, upload_id: &str, part: PartInfo) -> Result<(), S3Error> {
        let mut upload = self.get_multipart_upload(upload_id)?;
        upload.parts.retain(|p| p.part_number != part.part_number);
        upload.parts.push(part);
        upload.parts.sort_by_key(|p| p.part_number);
        let tree = self.db.open_tree(MULTIPART_TREE).map_err(|e| S3Error::InternalError(e.to_string()))?;
        let json = serde_json::to_vec(&upload).map_err(|e| S3Error::InternalError(e.to_string()))?;
        tree.insert(upload_id, json).map_err(|e| S3Error::InternalError(e.to_string()))?;
        Ok(())
    }

    pub fn count_multipart_uploads(&self) -> Result<usize, S3Error> {
        let tree = self.db.open_tree(MULTIPART_TREE).map_err(|e| S3Error::InternalError(e.to_string()))?;
        Ok(tree.len())
    }

    pub fn list_multipart_uploads(&self) -> Result<Vec<MultipartUpload>, S3Error> {
        let tree = self.db.open_tree(MULTIPART_TREE).map_err(|e| S3Error::InternalError(e.to_string()))?;
        let mut uploads = Vec::new();
        for item in tree.iter() {
            let (_, val) = item.map_err(|e| S3Error::InternalError(e.to_string()))?;
            let upload: MultipartUpload =
                serde_json::from_slice(&val).map_err(|e| S3Error::InternalError(e.to_string()))?;
            uploads.push(upload);
        }
        Ok(uploads)
    }

    pub fn delete_multipart_upload(&self, upload_id: &str) -> Result<(), S3Error> {
        let tree = self.db.open_tree(MULTIPART_TREE).map_err(|e| S3Error::InternalError(e.to_string()))?;
        tree.remove(upload_id).map_err(|e| S3Error::InternalError(e.to_string()))?;
        Ok(())
    }

    // --- Lifecycle configuration operations ---

    pub fn put_lifecycle_configuration(&self, bucket: &str, config: &LifecycleConfiguration) -> Result<(), S3Error> {
        let _ = self.get_bucket(bucket)?;
        let tree = self.db.open_tree(LIFECYCLE_TREE).map_err(|e| S3Error::InternalError(e.to_string()))?;
        let json = serde_json::to_vec(config).map_err(|e| S3Error::InternalError(e.to_string()))?;
        tree.insert(bucket, json).map_err(|e| S3Error::InternalError(e.to_string()))?;
        Ok(())
    }

    pub fn get_lifecycle_configuration(&self, bucket: &str) -> Result<LifecycleConfiguration, S3Error> {
        let _ = self.get_bucket(bucket)?;
        let tree = self.db.open_tree(LIFECYCLE_TREE).map_err(|e| S3Error::InternalError(e.to_string()))?;
        match tree.get(bucket).map_err(|e| S3Error::InternalError(e.to_string()))? {
            Some(bytes) => serde_json::from_slice(&bytes).map_err(|e| S3Error::InternalError(e.to_string())),
            None => Err(S3Error::NoSuchLifecycleConfiguration),
        }
    }

    pub fn delete_lifecycle_configuration(&self, bucket: &str) -> Result<(), S3Error> {
        let _ = self.get_bucket(bucket)?;
        let tree = self.db.open_tree(LIFECYCLE_TREE).map_err(|e| S3Error::InternalError(e.to_string()))?;
        tree.remove(bucket).map_err(|e| S3Error::InternalError(e.to_string()))?;
        Ok(())
    }

    pub fn list_lifecycle_configurations(&self) -> Result<Vec<(String, LifecycleConfiguration)>, S3Error> {
        let tree = self.db.open_tree(LIFECYCLE_TREE).map_err(|e| S3Error::InternalError(e.to_string()))?;
        let mut results = Vec::new();
        for item in tree.iter() {
            let (key, val) = item.map_err(|e| S3Error::InternalError(e.to_string()))?;
            let bucket = String::from_utf8_lossy(&key).into_owned();
            let config: LifecycleConfiguration = serde_json::from_slice(&val)
                .map_err(|e| S3Error::InternalError(e.to_string()))?;
            results.push((bucket, config));
        }
        Ok(results)
    }

    // --- Bucket policy operations ---

    pub fn put_bucket_policy(&self, bucket: &str, policy: &BucketPolicy) -> Result<(), S3Error> {
        let _ = self.get_bucket(bucket)?;
        let tree = self.db.open_tree(POLICIES_TREE).map_err(|e| S3Error::InternalError(e.to_string()))?;
        let json = serde_json::to_vec(policy).map_err(|e| S3Error::InternalError(e.to_string()))?;
        tree.insert(bucket, json).map_err(|e| S3Error::InternalError(e.to_string()))?;
        Ok(())
    }

    pub fn get_bucket_policy(&self, bucket: &str) -> Result<BucketPolicy, S3Error> {
        let _ = self.get_bucket(bucket)?;
        let tree = self.db.open_tree(POLICIES_TREE).map_err(|e| S3Error::InternalError(e.to_string()))?;
        match tree.get(bucket).map_err(|e| S3Error::InternalError(e.to_string()))? {
            Some(bytes) => serde_json::from_slice(&bytes).map_err(|e| S3Error::InternalError(e.to_string())),
            None => Err(S3Error::NoSuchBucketPolicy),
        }
    }

    pub fn delete_bucket_policy(&self, bucket: &str) -> Result<(), S3Error> {
        let _ = self.get_bucket(bucket)?;
        let tree = self.db.open_tree(POLICIES_TREE).map_err(|e| S3Error::InternalError(e.to_string()))?;
        tree.remove(bucket).map_err(|e| S3Error::InternalError(e.to_string()))?;
        Ok(())
    }

    // --- CORS configuration operations ---

    pub fn put_cors_configuration(&self, bucket: &str, config: &CorsConfiguration) -> Result<(), S3Error> {
        let _ = self.get_bucket(bucket)?;
        let tree = self.db.open_tree(CORS_TREE).map_err(|e| S3Error::InternalError(e.to_string()))?;
        let json = serde_json::to_vec(config).map_err(|e| S3Error::InternalError(e.to_string()))?;
        tree.insert(bucket, json).map_err(|e| S3Error::InternalError(e.to_string()))?;
        Ok(())
    }

    pub fn get_cors_configuration(&self, bucket: &str) -> Result<CorsConfiguration, S3Error> {
        let _ = self.get_bucket(bucket)?;
        let tree = self.db.open_tree(CORS_TREE).map_err(|e| S3Error::InternalError(e.to_string()))?;
        match tree.get(bucket).map_err(|e| S3Error::InternalError(e.to_string()))? {
            Some(bytes) => serde_json::from_slice(&bytes).map_err(|e| S3Error::InternalError(e.to_string())),
            None => Err(S3Error::NoSuchCORSConfiguration),
        }
    }

    pub fn delete_cors_configuration(&self, bucket: &str) -> Result<(), S3Error> {
        let _ = self.get_bucket(bucket)?;
        let tree = self.db.open_tree(CORS_TREE).map_err(|e| S3Error::InternalError(e.to_string()))?;
        tree.remove(bucket).map_err(|e| S3Error::InternalError(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store() -> (MetadataStore, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let store = MetadataStore::open(dir.path()).unwrap();
        (store, dir)
    }

    #[test]
    fn test_bucket_crud() {
        let (store, _dir) = temp_store();
        let meta = store.create_bucket("test-bucket").unwrap();
        assert_eq!(meta.name, "test-bucket");

        let fetched = store.get_bucket("test-bucket").unwrap();
        assert_eq!(fetched.name, "test-bucket");

        let list = store.list_buckets().unwrap();
        assert_eq!(list.len(), 1);

        store.delete_bucket("test-bucket").unwrap();
        assert!(matches!(store.get_bucket("test-bucket"), Err(S3Error::NoSuchBucket)));
    }

    #[test]
    fn test_bucket_already_exists() {
        let (store, _dir) = temp_store();
        store.create_bucket("dup-bucket").unwrap();
        assert!(matches!(store.create_bucket("dup-bucket"), Err(S3Error::BucketAlreadyExists)));
    }

    #[test]
    fn test_delete_nonempty_bucket() {
        let (store, _dir) = temp_store();
        store.create_bucket("bucket1").unwrap();
        store.put_object_meta(&ObjectMeta {
            bucket: "bucket1".into(),
            key: "file.txt".into(),
            size: 10,
            etag: "abc".into(),
            content_type: "text/plain".into(),
            last_modified: Utc::now(),
            public: false,
        }).unwrap();
        assert!(matches!(store.delete_bucket("bucket1"), Err(S3Error::BucketNotEmpty)));
    }

    #[test]
    fn test_object_meta_crud() {
        let (store, _dir) = temp_store();
        store.create_bucket("test-bkt").unwrap();
        let meta = ObjectMeta {
            bucket: "test-bkt".into(),
            key: "k".into(),
            size: 42,
            etag: "etag".into(),
            content_type: "application/octet-stream".into(),
            last_modified: Utc::now(),
            public: false,
        };
        store.put_object_meta(&meta).unwrap();
        let fetched = store.get_object_meta("test-bkt", "k").unwrap();
        assert_eq!(fetched.size, 42);
        store.delete_object_meta("test-bkt", "k").unwrap();
        assert!(matches!(store.get_object_meta("test-bkt", "k"), Err(S3Error::NoSuchKey)));
    }

    #[test]
    fn test_list_objects_prefix() {
        let (store, _dir) = temp_store();
        store.create_bucket("test-bkt").unwrap();
        for key in ["photos/a.jpg", "photos/b.jpg", "docs/c.pdf"] {
            store.put_object_meta(&ObjectMeta {
                bucket: "test-bkt".into(),
                key: key.into(),
                size: 1,
                etag: "e".into(),
                content_type: "".into(),
                last_modified: Utc::now(),
                public: false,
            }).unwrap();
        }
        let resp = store.list_objects_v2(&ListObjectsV2Request {
            bucket: "test-bkt".into(),
            prefix: "photos/".into(),
            delimiter: String::new(),
            max_keys: 1000,
            continuation_token: None,
            start_after: None,
        }).unwrap();
        assert_eq!(resp.contents.len(), 2);
    }

    #[test]
    fn test_list_objects_delimiter() {
        let (store, _dir) = temp_store();
        store.create_bucket("test-bkt").unwrap();
        for key in ["photos/a.jpg", "photos/b.jpg", "docs/c.pdf", "root.txt"] {
            store.put_object_meta(&ObjectMeta {
                bucket: "test-bkt".into(),
                key: key.into(),
                size: 1,
                etag: "e".into(),
                content_type: "".into(),
                last_modified: Utc::now(),
                public: false,
            }).unwrap();
        }
        let resp = store.list_objects_v2(&ListObjectsV2Request {
            bucket: "test-bkt".into(),
            prefix: String::new(),
            delimiter: "/".into(),
            max_keys: 1000,
            continuation_token: None,
            start_after: None,
        }).unwrap();
        assert_eq!(resp.contents.len(), 1); // root.txt
        assert_eq!(resp.common_prefixes.len(), 2); // docs/, photos/
    }

    #[test]
    fn test_list_objects_pagination() {
        let (store, _dir) = temp_store();
        store.create_bucket("test-bkt").unwrap();
        for i in 0..5 {
            store.put_object_meta(&ObjectMeta {
                bucket: "test-bkt".into(),
                key: format!("key{}", i),
                size: 1,
                etag: "e".into(),
                content_type: "".into(),
                last_modified: Utc::now(),
                public: false,
            }).unwrap();
        }
        let resp = store.list_objects_v2(&ListObjectsV2Request {
            bucket: "test-bkt".into(),
            prefix: String::new(),
            delimiter: String::new(),
            max_keys: 2,
            continuation_token: None,
            start_after: None,
        }).unwrap();
        assert_eq!(resp.contents.len(), 2);
        assert!(resp.is_truncated);
        assert!(resp.next_continuation_token.is_some());

        let resp2 = store.list_objects_v2(&ListObjectsV2Request {
            bucket: "test-bkt".into(),
            prefix: String::new(),
            delimiter: String::new(),
            max_keys: 2,
            continuation_token: resp.next_continuation_token,
            start_after: None,
        }).unwrap();
        assert_eq!(resp2.contents.len(), 2);
    }

    #[test]
    fn test_object_tagging_crud() {
        let (store, _dir) = temp_store();
        store.create_bucket("test-bkt").unwrap();
        store.put_object_meta(&ObjectMeta {
            bucket: "test-bkt".into(),
            key: "k".into(),
            size: 10,
            etag: "e".into(),
            content_type: "".into(),
            last_modified: Utc::now(),
            public: false,
        }).unwrap();

        // No tags initially
        let tags = store.get_object_tagging("test-bkt", "k").unwrap();
        assert!(tags.is_empty());

        // Put tags
        let mut tags = HashMap::new();
        tags.insert("env".into(), "prod".into());
        tags.insert("team".into(), "eng".into());
        store.put_object_tagging("test-bkt", "k", &tags).unwrap();

        // Get tags
        let fetched = store.get_object_tagging("test-bkt", "k").unwrap();
        assert_eq!(fetched.len(), 2);
        assert_eq!(fetched.get("env").unwrap(), "prod");

        // Delete tags
        store.delete_object_tagging("test-bkt", "k").unwrap();
        let fetched = store.get_object_tagging("test-bkt", "k").unwrap();
        assert!(fetched.is_empty());
    }

    #[test]
    fn test_tagging_cleanup_on_object_delete() {
        let (store, _dir) = temp_store();
        store.create_bucket("test-bkt").unwrap();
        store.put_object_meta(&ObjectMeta {
            bucket: "test-bkt".into(),
            key: "k".into(),
            size: 10,
            etag: "e".into(),
            content_type: "".into(),
            last_modified: Utc::now(),
            public: false,
        }).unwrap();

        let mut tags = HashMap::new();
        tags.insert("foo".into(), "bar".into());
        store.put_object_tagging("test-bkt", "k", &tags).unwrap();

        // Delete object — tags should be cleaned up
        store.delete_object_meta("test-bkt", "k").unwrap();

        // Re-create object and verify tags are gone
        store.put_object_meta(&ObjectMeta {
            bucket: "test-bkt".into(),
            key: "k".into(),
            size: 10,
            etag: "e".into(),
            content_type: "".into(),
            last_modified: Utc::now(),
            public: false,
        }).unwrap();
        let fetched = store.get_object_tagging("test-bkt", "k").unwrap();
        assert!(fetched.is_empty());
    }

    #[test]
    fn test_credential_crud() {
        let (store, _dir) = temp_store();
        let cred = store.create_credential("AKID", "SECRET", "test key").unwrap();
        assert_eq!(cred.access_key_id, "AKID");
        assert!(cred.active);

        let fetched = store.get_credential("AKID").unwrap();
        assert_eq!(fetched.secret_access_key, "SECRET");

        let list = store.list_credentials().unwrap();
        assert_eq!(list.len(), 1);

        store.revoke_credential("AKID").unwrap();
        let revoked = store.get_credential("AKID").unwrap();
        assert!(!revoked.active);
    }

    #[test]
    fn test_multipart_lifecycle() {
        let (store, _dir) = temp_store();
        let upload = MultipartUpload {
            upload_id: "up1".into(),
            bucket: "test-bkt".into(),
            key: "k".into(),
            created: Utc::now(),
            parts: vec![],
        };
        store.create_multipart_upload(&upload).unwrap();

        store.add_part_to_upload("up1", PartInfo {
            part_number: 1,
            etag: "e1".into(),
            size: 100,
            last_modified: Utc::now(),
        }).unwrap();

        let fetched = store.get_multipart_upload("up1").unwrap();
        assert_eq!(fetched.parts.len(), 1);

        store.delete_multipart_upload("up1").unwrap();
        assert!(matches!(store.get_multipart_upload("up1"), Err(S3Error::NoSuchUpload)));
    }

    #[test]
    fn test_lifecycle_crud() {
        use crate::s3::types::{LifecycleConfiguration, LifecycleRule, LifecycleStatus};
        let (store, _dir) = temp_store();
        store.create_bucket("test-bkt").unwrap();

        // No lifecycle initially
        assert!(matches!(
            store.get_lifecycle_configuration("test-bkt"),
            Err(S3Error::NoSuchLifecycleConfiguration)
        ));

        let config = LifecycleConfiguration {
            rules: vec![LifecycleRule {
                id: "expire-logs".into(),
                prefix: "logs/".into(),
                status: LifecycleStatus::Enabled,
                expiration_days: 30,
                expiration_date: None,
                tags: vec![],
            }],
        };
        store.put_lifecycle_configuration("test-bkt", &config).unwrap();

        let fetched = store.get_lifecycle_configuration("test-bkt").unwrap();
        assert_eq!(fetched.rules.len(), 1);
        assert_eq!(fetched.rules[0].expiration_days, 30);

        let all = store.list_lifecycle_configurations().unwrap();
        assert_eq!(all.len(), 1);

        store.delete_lifecycle_configuration("test-bkt").unwrap();
        assert!(matches!(
            store.get_lifecycle_configuration("test-bkt"),
            Err(S3Error::NoSuchLifecycleConfiguration)
        ));
    }

    #[test]
    fn test_policy_crud() {
        use crate::s3::types::{BucketPolicy, OneOrMany, PolicyEffect, PolicyPrincipal, PolicyStatement};
        let (store, _dir) = temp_store();
        store.create_bucket("test-bkt").unwrap();

        assert!(matches!(
            store.get_bucket_policy("test-bkt"),
            Err(S3Error::NoSuchBucketPolicy)
        ));

        let policy = BucketPolicy {
            version: "2012-10-17".into(),
            statements: vec![PolicyStatement {
                sid: Some("AllowAnon".into()),
                effect: PolicyEffect::Allow,
                principal: PolicyPrincipal::Wildcard("*".into()),
                action: OneOrMany::One("s3:GetObject".into()),
                resource: OneOrMany::One("arn:aws:s3:::test-bkt/*".into()),
                condition: None,
            }],
        };
        store.put_bucket_policy("test-bkt", &policy).unwrap();

        let fetched = store.get_bucket_policy("test-bkt").unwrap();
        assert_eq!(fetched.statements.len(), 1);

        store.delete_bucket_policy("test-bkt").unwrap();
        assert!(matches!(
            store.get_bucket_policy("test-bkt"),
            Err(S3Error::NoSuchBucketPolicy)
        ));
    }

    #[test]
    fn test_delete_bucket_cleans_lifecycle_and_policy() {
        use crate::s3::types::{BucketPolicy, LifecycleConfiguration, LifecycleRule, LifecycleStatus, OneOrMany, PolicyEffect, PolicyPrincipal, PolicyStatement};
        let (store, _dir) = temp_store();
        store.create_bucket("test-bkt").unwrap();

        let config = LifecycleConfiguration {
            rules: vec![LifecycleRule {
                id: "r".into(),
                prefix: "".into(),
                status: LifecycleStatus::Enabled,
                expiration_days: 1,
                expiration_date: None,
                tags: vec![],
            }],
        };
        store.put_lifecycle_configuration("test-bkt", &config).unwrap();

        let policy = BucketPolicy {
            version: "2012-10-17".into(),
            statements: vec![PolicyStatement {
                sid: None,
                effect: PolicyEffect::Allow,
                principal: PolicyPrincipal::Wildcard("*".into()),
                action: OneOrMany::One("s3:GetObject".into()),
                resource: OneOrMany::One("arn:aws:s3:::test-bkt/*".into()),
                condition: None,
            }],
        };
        store.put_bucket_policy("test-bkt", &policy).unwrap();

        store.delete_bucket("test-bkt").unwrap();

        // Re-create bucket and verify lifecycle/policy are gone
        store.create_bucket("test-bkt").unwrap();
        assert!(matches!(
            store.get_lifecycle_configuration("test-bkt"),
            Err(S3Error::NoSuchLifecycleConfiguration)
        ));
        assert!(matches!(
            store.get_bucket_policy("test-bkt"),
            Err(S3Error::NoSuchBucketPolicy)
        ));
    }

    #[test]
    fn test_cors_crud() {
        use crate::s3::types::{CorsConfiguration, CorsRule};
        let (store, _dir) = temp_store();
        store.create_bucket("test-bkt").unwrap();

        assert!(matches!(
            store.get_cors_configuration("test-bkt"),
            Err(S3Error::NoSuchCORSConfiguration)
        ));

        let config = CorsConfiguration {
            rules: vec![CorsRule {
                id: Some("rule1".into()),
                allowed_origins: vec!["https://example.com".into()],
                allowed_methods: vec!["GET".into(), "PUT".into()],
                allowed_headers: vec!["*".into()],
                expose_headers: vec![],
                max_age_seconds: Some(3600),
            }],
        };
        store.put_cors_configuration("test-bkt", &config).unwrap();

        let fetched = store.get_cors_configuration("test-bkt").unwrap();
        assert_eq!(fetched.rules.len(), 1);
        assert_eq!(fetched.rules[0].allowed_origins, vec!["https://example.com"]);

        store.delete_cors_configuration("test-bkt").unwrap();
        assert!(matches!(
            store.get_cors_configuration("test-bkt"),
            Err(S3Error::NoSuchCORSConfiguration)
        ));
    }

    #[test]
    fn test_delete_bucket_cleans_cors() {
        use crate::s3::types::{CorsConfiguration, CorsRule};
        let (store, _dir) = temp_store();
        store.create_bucket("test-bkt").unwrap();

        let config = CorsConfiguration {
            rules: vec![CorsRule {
                id: None,
                allowed_origins: vec!["*".into()],
                allowed_methods: vec!["GET".into()],
                allowed_headers: vec![],
                expose_headers: vec![],
                max_age_seconds: None,
            }],
        };
        store.put_cors_configuration("test-bkt", &config).unwrap();

        store.delete_bucket("test-bkt").unwrap();

        store.create_bucket("test-bkt").unwrap();
        assert!(matches!(
            store.get_cors_configuration("test-bkt"),
            Err(S3Error::NoSuchCORSConfiguration)
        ));
    }

    #[test]
    fn test_list_multipart_uploads() {
        let (store, _dir) = temp_store();

        // Empty initially
        let uploads = store.list_multipart_uploads().unwrap();
        assert!(uploads.is_empty());

        // Create two uploads
        for id in ["up1", "up2"] {
            store.create_multipart_upload(&MultipartUpload {
                upload_id: id.into(),
                bucket: "test-bkt".into(),
                key: "k".into(),
                created: Utc::now(),
                parts: vec![],
            }).unwrap();
        }

        let uploads = store.list_multipart_uploads().unwrap();
        assert_eq!(uploads.len(), 2);

        // Delete one, list again
        store.delete_multipart_upload("up1").unwrap();
        let uploads = store.list_multipart_uploads().unwrap();
        assert_eq!(uploads.len(), 1);
        assert_eq!(uploads[0].upload_id, "up2");
    }
}
