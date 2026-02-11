use crate::error::S3Error;
use md5::{Digest, Md5};
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;

#[derive(Clone)]
pub struct FileStore {
    data_dir: PathBuf,
}

impl FileStore {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            data_dir: data_dir.to_path_buf(),
        }
    }

    pub fn bucket_path(&self, bucket: &str) -> PathBuf {
        self.data_dir.join(bucket)
    }

    pub fn object_path(&self, bucket: &str, key: &str) -> PathBuf {
        self.data_dir.join(bucket).join(key)
    }

    /// Validate that a resolved path stays within the expected base directory.
    /// Prevents path traversal attacks via `..` or absolute path components.
    fn validate_path(&self, path: &Path, base: &Path) -> Result<(), S3Error> {
        // Normalize away `.` and `..` without requiring the path to exist.
        // We use a component-based approach since the path may not exist yet
        // (canonicalize requires the file to exist).
        let normalized = normalize_path(path);
        let norm_base = normalize_path(base);
        if !normalized.starts_with(&norm_base) {
            return Err(S3Error::AccessDenied);
        }
        Ok(())
    }

    fn safe_object_path(&self, bucket: &str, key: &str) -> Result<PathBuf, S3Error> {
        validate_name(bucket)?;
        validate_key(key)?;
        let path = self.object_path(bucket, key);
        self.validate_path(&path, &self.bucket_path(bucket))?;
        Ok(path)
    }

    fn safe_bucket_path(&self, bucket: &str) -> Result<PathBuf, S3Error> {
        validate_name(bucket)?;
        let path = self.bucket_path(bucket);
        self.validate_path(&path, &self.data_dir)?;
        Ok(path)
    }

    fn multipart_dir(&self, upload_id: &str) -> PathBuf {
        self.data_dir.join(".multipart").join(upload_id)
    }

    fn part_path(&self, upload_id: &str, part_number: u32) -> PathBuf {
        self.multipart_dir(upload_id)
            .join(format!("part-{}", part_number))
    }

    pub async fn create_bucket_dir(&self, bucket: &str) -> Result<(), S3Error> {
        let path = self.safe_bucket_path(bucket)?;
        fs::create_dir_all(&path)
            .await
            .map_err(|e| S3Error::InternalError(e.to_string()))
    }

    pub async fn delete_bucket_dir(&self, bucket: &str) -> Result<(), S3Error> {
        let path = self.safe_bucket_path(bucket)?;
        if path.exists() {
            fs::remove_dir_all(&path)
                .await
                .map_err(|e| S3Error::InternalError(e.to_string()))?;
        }
        Ok(())
    }

    /// Write object data atomically via temp file + rename. Returns (size, md5_hex).
    pub async fn write_object(
        &self,
        bucket: &str,
        key: &str,
        data: &[u8],
    ) -> Result<(u64, String), S3Error> {
        let target = self.safe_object_path(bucket, key)?;
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| S3Error::InternalError(e.to_string()))?;
        }

        let temp_path = target.with_extension(format!("tmp.{}", Uuid::new_v4()));

        let mut file = fs::File::create(&temp_path)
            .await
            .map_err(|e| S3Error::InternalError(e.to_string()))?;

        file.write_all(data)
            .await
            .map_err(|e| S3Error::InternalError(e.to_string()))?;

        file.flush()
            .await
            .map_err(|e| S3Error::InternalError(e.to_string()))?;

        fs::rename(&temp_path, &target)
            .await
            .map_err(|e| S3Error::InternalError(e.to_string()))?;

        let size = data.len() as u64;
        let etag = hex::encode(Md5::digest(data));
        Ok((size, etag))
    }

    /// Stream-write object from an async reader. Returns (size, md5_hex).
    pub async fn write_object_stream<R: tokio::io::AsyncRead + Unpin>(
        &self,
        bucket: &str,
        key: &str,
        reader: &mut R,
    ) -> Result<(u64, String), S3Error> {
        let target = self.safe_object_path(bucket, key)?;
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| S3Error::InternalError(e.to_string()))?;
        }

        let temp_path = target.with_extension(format!("tmp.{}", Uuid::new_v4()));
        let mut file = fs::File::create(&temp_path)
            .await
            .map_err(|e| S3Error::InternalError(e.to_string()))?;

        let mut hasher = Md5::new();
        let mut total_size: u64 = 0;
        let mut buf = vec![0u8; 64 * 1024];

        loop {
            let n = reader
                .read(&mut buf)
                .await
                .map_err(|e| S3Error::InternalError(e.to_string()))?;
            if n == 0 {
                break;
            }
            file.write_all(&buf[..n])
                .await
                .map_err(|e| S3Error::InternalError(e.to_string()))?;
            hasher.update(&buf[..n]);
            total_size += n as u64;
        }

        file.flush()
            .await
            .map_err(|e| S3Error::InternalError(e.to_string()))?;

        fs::rename(&temp_path, &target)
            .await
            .map_err(|e| S3Error::InternalError(e.to_string()))?;

        let etag = hex::encode(hasher.finalize());
        Ok((total_size, etag))
    }

    pub async fn read_object(&self, bucket: &str, key: &str) -> Result<Vec<u8>, S3Error> {
        let path = self.safe_object_path(bucket, key)?;
        fs::read(&path)
            .await
            .map_err(|_| S3Error::NoSuchKey)
    }

    pub fn open_object_file(
        &self,
        bucket: &str,
        key: &str,
    ) -> Result<PathBuf, S3Error> {
        self.safe_object_path(bucket, key)
    }

    pub async fn copy_object(
        &self,
        src_bucket: &str,
        src_key: &str,
        dst_bucket: &str,
        dst_key: &str,
    ) -> Result<(u64, String), S3Error> {
        let data = self.read_object(src_bucket, src_key).await?;
        self.write_object(dst_bucket, dst_key, &data).await
    }

    pub async fn delete_object(&self, bucket: &str, key: &str) -> Result<(), S3Error> {
        let path = self.safe_object_path(bucket, key)?;
        if path.exists() {
            fs::remove_file(&path)
                .await
                .map_err(|e| S3Error::InternalError(e.to_string()))?;
        }
        Ok(())
    }

    // --- Multipart ---

    pub async fn write_part(
        &self,
        upload_id: &str,
        part_number: u32,
        data: &[u8],
    ) -> Result<(u64, String), S3Error> {
        let dir = self.multipart_dir(upload_id);
        fs::create_dir_all(&dir)
            .await
            .map_err(|e| S3Error::InternalError(e.to_string()))?;

        let path = self.part_path(upload_id, part_number);
        fs::write(&path, data)
            .await
            .map_err(|e| S3Error::InternalError(e.to_string()))?;

        let size = data.len() as u64;
        let etag = hex::encode(Md5::digest(data));
        Ok((size, etag))
    }

    pub async fn write_part_stream<R: tokio::io::AsyncRead + Unpin>(
        &self,
        upload_id: &str,
        part_number: u32,
        reader: &mut R,
    ) -> Result<(u64, String), S3Error> {
        let dir = self.multipart_dir(upload_id);
        fs::create_dir_all(&dir)
            .await
            .map_err(|e| S3Error::InternalError(e.to_string()))?;

        let path = self.part_path(upload_id, part_number);
        let mut file = fs::File::create(&path)
            .await
            .map_err(|e| S3Error::InternalError(e.to_string()))?;

        let mut hasher = Md5::new();
        let mut total_size: u64 = 0;
        let mut buf = vec![0u8; 64 * 1024];

        loop {
            let n = reader
                .read(&mut buf)
                .await
                .map_err(|e| S3Error::InternalError(e.to_string()))?;
            if n == 0 {
                break;
            }
            file.write_all(&buf[..n])
                .await
                .map_err(|e| S3Error::InternalError(e.to_string()))?;
            hasher.update(&buf[..n]);
            total_size += n as u64;
        }

        file.flush()
            .await
            .map_err(|e| S3Error::InternalError(e.to_string()))?;

        let etag = hex::encode(hasher.finalize());
        Ok((total_size, etag))
    }

    /// Assemble parts into the final object. Returns (size, multipart_etag).
    pub async fn assemble_parts(
        &self,
        bucket: &str,
        key: &str,
        upload_id: &str,
        part_numbers: &[u32],
    ) -> Result<(u64, String), S3Error> {
        let target = self.safe_object_path(bucket, key)?;
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| S3Error::InternalError(e.to_string()))?;
        }

        let temp_path = target.with_extension(format!("tmp.{}", Uuid::new_v4()));
        let mut file = fs::File::create(&temp_path)
            .await
            .map_err(|e| S3Error::InternalError(e.to_string()))?;

        let mut total_size: u64 = 0;
        let mut part_md5s: Vec<Vec<u8>> = Vec::new();

        for &pn in part_numbers {
            let part_path = self.part_path(upload_id, pn);
            let data = fs::read(&part_path)
                .await
                .map_err(|_| S3Error::InvalidPart)?;
            file.write_all(&data)
                .await
                .map_err(|e| S3Error::InternalError(e.to_string()))?;
            total_size += data.len() as u64;
            part_md5s.push(Md5::digest(&data).to_vec());
        }

        file.flush()
            .await
            .map_err(|e| S3Error::InternalError(e.to_string()))?;

        fs::rename(&temp_path, &target)
            .await
            .map_err(|e| S3Error::InternalError(e.to_string()))?;

        // Multipart ETag: md5(concat(part_md5s))-N
        let mut combined = Vec::new();
        for md5 in &part_md5s {
            combined.extend_from_slice(md5);
        }
        let etag = format!("{}-{}", hex::encode(Md5::digest(&combined)), part_numbers.len());

        Ok((total_size, etag))
    }

    pub async fn cleanup_multipart(&self, upload_id: &str) -> Result<(), S3Error> {
        let dir = self.multipart_dir(upload_id);
        if dir.exists() {
            fs::remove_dir_all(&dir)
                .await
                .map_err(|e| S3Error::InternalError(e.to_string()))?;
        }
        Ok(())
    }
}

/// Normalize a path by resolving `.` and `..` components without touching the filesystem.
fn normalize_path(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                result.pop();
            }
            Component::CurDir => {}
            other => result.push(other),
        }
    }
    result
}

/// Validate a bucket name: reject path traversal components and null bytes.
fn validate_name(name: &str) -> Result<(), S3Error> {
    if name.is_empty()
        || name.contains('\0')
        || name.contains('/')
        || name.contains('\\')
        || name == "."
        || name == ".."
        || name.starts_with('.')
    {
        return Err(S3Error::InvalidArgument(format!(
            "Invalid bucket name: {}",
            name
        )));
    }
    Ok(())
}

/// Validate an object key: reject null bytes and leading slashes.
fn validate_key(key: &str) -> Result<(), S3Error> {
    if key.is_empty() || key.contains('\0') {
        return Err(S3Error::InvalidArgument("Invalid object key".into()));
    }
    // Reject keys that would escape the bucket directory
    for component in Path::new(key).components() {
        if matches!(component, std::path::Component::ParentDir) {
            return Err(S3Error::AccessDenied);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store() -> (FileStore, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let store = FileStore::new(dir.path());
        (store, dir)
    }

    #[tokio::test]
    async fn test_write_and_read_object() {
        let (store, _dir) = temp_store();
        store.create_bucket_dir("b").await.unwrap();
        let data = b"hello world";
        let (size, etag) = store.write_object("b", "key.txt", data).await.unwrap();
        assert_eq!(size, 11);
        assert!(!etag.is_empty());
        let read = store.read_object("b", "key.txt").await.unwrap();
        assert_eq!(read, data);
    }

    #[tokio::test]
    async fn test_write_atomic() {
        let (store, dir) = temp_store();
        store.create_bucket_dir("b").await.unwrap();
        store.write_object("b", "f.txt", b"data").await.unwrap();
        // No temp files should remain
        let bucket_dir = dir.path().join("b");
        let entries: Vec<_> = std::fs::read_dir(&bucket_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].file_name().to_str().unwrap(), "f.txt");
    }

    #[tokio::test]
    async fn test_delete_object() {
        let (store, _dir) = temp_store();
        store.create_bucket_dir("b").await.unwrap();
        store.write_object("b", "k", b"data").await.unwrap();
        store.delete_object("b", "k").await.unwrap();
        assert!(store.read_object("b", "k").await.is_err());
    }

    #[tokio::test]
    async fn test_nested_key_paths() {
        let (store, _dir) = temp_store();
        store.create_bucket_dir("b").await.unwrap();
        store.write_object("b", "a/b/c/file.txt", b"nested").await.unwrap();
        let read = store.read_object("b", "a/b/c/file.txt").await.unwrap();
        assert_eq!(read, b"nested");
    }

    #[tokio::test]
    async fn test_bucket_dir_operations() {
        let (store, dir) = temp_store();
        store.create_bucket_dir("test").await.unwrap();
        assert!(dir.path().join("test").exists());
        store.delete_bucket_dir("test").await.unwrap();
        assert!(!dir.path().join("test").exists());
    }

    #[tokio::test]
    async fn test_copy_object() {
        let (store, _dir) = temp_store();
        store.create_bucket_dir("b").await.unwrap();
        store.write_object("b", "src.txt", b"copy me").await.unwrap();
        let (size, etag) = store.copy_object("b", "src.txt", "b", "dst.txt").await.unwrap();
        assert_eq!(size, 7);
        assert!(!etag.is_empty());
        let data = store.read_object("b", "dst.txt").await.unwrap();
        assert_eq!(data, b"copy me");
    }

    #[tokio::test]
    async fn test_copy_object_cross_bucket() {
        let (store, _dir) = temp_store();
        store.create_bucket_dir("src-b").await.unwrap();
        store.create_bucket_dir("dst-b").await.unwrap();
        store.write_object("src-b", "file.txt", b"cross").await.unwrap();
        let (size, _) = store.copy_object("src-b", "file.txt", "dst-b", "file.txt").await.unwrap();
        assert_eq!(size, 5);
        let data = store.read_object("dst-b", "file.txt").await.unwrap();
        assert_eq!(data, b"cross");
    }

    #[tokio::test]
    async fn test_path_traversal_rejected() {
        let (store, _dir) = temp_store();
        store.create_bucket_dir("b").await.unwrap();
        // Attempt path traversal via object key
        let result = store.write_object("b", "../../../etc/passwd", b"evil").await;
        assert!(result.is_err());
        let result = store.write_object("b", "foo/../../bar", b"evil").await;
        assert!(result.is_err());
        // Attempt path traversal via bucket name
        let result = store.create_bucket_dir("../escape").await;
        assert!(result.is_err());
        // Null byte in key
        let result = store.write_object("b", "file\0.txt", b"evil").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_multipart_assembly() {
        let (store, _dir) = temp_store();
        store.create_bucket_dir("b").await.unwrap();
        let uid = "test-upload";
        store.write_part(uid, 1, b"part1-").await.unwrap();
        store.write_part(uid, 2, b"part2-").await.unwrap();
        store.write_part(uid, 3, b"part3").await.unwrap();

        let (size, etag) = store.assemble_parts("b", "assembled.txt", uid, &[1, 2, 3]).await.unwrap();
        assert_eq!(size, 17); // "part1-" + "part2-" + "part3" = 17 bytes
        assert!(etag.ends_with("-3"));

        let content = store.read_object("b", "assembled.txt").await.unwrap();
        assert_eq!(content, b"part1-part2-part3");

        store.cleanup_multipart(uid).await.unwrap();
    }
}
