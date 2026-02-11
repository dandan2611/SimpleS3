use crate::error::S3Error;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

type HmacSha256 = Hmac<Sha256>;

/// Parsed Authorization header for AWS SigV4
#[derive(Debug)]
pub struct SigV4Auth {
    pub access_key_id: String,
    pub date: String,       // YYYYMMDD
    pub region: String,
    pub signed_headers: Vec<String>,
    pub signature: String,
}

pub fn parse_auth_header(header: &str) -> Result<SigV4Auth, S3Error> {
    // AWS4-HMAC-SHA256 Credential=AKID/20230101/us-east-1/s3/aws4_request,
    // SignedHeaders=host;x-amz-content-sha256;x-amz-date,
    // Signature=abcdef...
    let header = header
        .strip_prefix("AWS4-HMAC-SHA256 ")
        .ok_or(S3Error::AccessDenied)?;

    let mut credential = None;
    let mut signed_headers = None;
    let mut signature = None;

    for part in header.split(',') {
        let part = part.trim();
        if let Some(val) = part.strip_prefix("Credential=") {
            credential = Some(val);
        } else if let Some(val) = part.strip_prefix("SignedHeaders=") {
            signed_headers = Some(val);
        } else if let Some(val) = part.strip_prefix("Signature=") {
            signature = Some(val);
        }
    }

    let credential = credential.ok_or(S3Error::AccessDenied)?;
    let signed_headers = signed_headers.ok_or(S3Error::AccessDenied)?;
    let signature = signature.ok_or(S3Error::AccessDenied)?;

    // Parse credential: AKID/20230101/us-east-1/s3/aws4_request
    let cred_parts: Vec<&str> = credential.split('/').collect();
    if cred_parts.len() != 5 {
        return Err(S3Error::AccessDenied);
    }

    Ok(SigV4Auth {
        access_key_id: cred_parts[0].to_string(),
        date: cred_parts[1].to_string(),
        region: cred_parts[2].to_string(),
        signed_headers: signed_headers.split(';').map(|s| s.to_string()).collect(),
        signature: signature.to_string(),
    })
}

pub fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC key");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

pub fn signing_key(secret: &str, date: &str, region: &str) -> Vec<u8> {
    let k_date = hmac_sha256(format!("AWS4{}", secret).as_bytes(), date.as_bytes());
    let k_region = hmac_sha256(&k_date, region.as_bytes());
    let k_service = hmac_sha256(&k_region, b"s3");
    hmac_sha256(&k_service, b"aws4_request")
}

/// Build the canonical request string
pub fn canonical_request(
    method: &str,
    uri: &str,
    query_string: &str,
    headers: &BTreeMap<String, String>,
    signed_headers: &[String],
    payload_hash: &str,
) -> String {
    let canonical_headers: String = signed_headers
        .iter()
        .map(|h| {
            let val = headers.get(h).map(|v| v.trim()).unwrap_or("");
            format!("{}:{}\n", h, val)
        })
        .collect();

    let signed_headers_str = signed_headers.join(";");

    format!(
        "{}\n{}\n{}\n{}\n{}\n{}",
        method, uri, query_string, canonical_headers, signed_headers_str, payload_hash
    )
}

/// Verify a SigV4 request. Returns Ok(access_key_id) on success.
pub fn verify_signature(
    method: &str,
    uri: &str,
    query_string: &str,
    headers: &BTreeMap<String, String>,
    auth: &SigV4Auth,
    secret_key: &str,
    payload_hash: &str,
) -> Result<(), S3Error> {
    let canon = canonical_request(method, uri, query_string, headers, &auth.signed_headers, payload_hash);

    let hash_canon = hex::encode(Sha256::digest(canon.as_bytes()));

    let scope = format!("{}/{}/s3/aws4_request", auth.date, auth.region);
    let string_to_sign = format!("AWS4-HMAC-SHA256\n{}\n{}\n{}",
        headers.get("x-amz-date").unwrap_or(&String::new()),
        scope,
        hash_canon
    );

    let key = signing_key(secret_key, &auth.date, &auth.region);
    let computed = hex::encode(hmac_sha256(&key, string_to_sign.as_bytes()));

    if constant_time_eq(computed.as_bytes(), auth.signature.as_bytes()) {
        Ok(())
    } else {
        Err(S3Error::SignatureDoesNotMatch)
    }
}

/// Verify a presigned URL signature.
pub fn verify_presigned_signature(
    method: &str,
    uri: &str,
    canonical_query: &str,
    headers: &BTreeMap<String, String>,
    signed_headers: &[String],
    date: &str,
    amz_date: &str,
    region: &str,
    secret_key: &str,
    signature: &str,
) -> Result<(), S3Error> {
    let canon = canonical_request(
        method,
        uri,
        canonical_query,
        headers,
        signed_headers,
        "UNSIGNED-PAYLOAD",
    );

    let hash_canon = hex::encode(Sha256::digest(canon.as_bytes()));
    let scope = format!("{}/{}/s3/aws4_request", date, region);
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{}\n{}\n{}",
        amz_date, scope, hash_canon
    );

    let key = signing_key(secret_key, date, region);
    let computed = hex::encode(hmac_sha256(&key, string_to_sign.as_bytes()));

    if constant_time_eq(computed.as_bytes(), signature.as_bytes()) {
        Ok(())
    } else {
        Err(S3Error::SignatureDoesNotMatch)
    }
}

/// Constant-time byte comparison to prevent timing attacks.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_auth_header() {
        let header = "AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/20150830/us-east-1/s3/aws4_request, SignedHeaders=host;x-amz-content-sha256;x-amz-date, Signature=aaaa";
        let auth = parse_auth_header(header).unwrap();
        assert_eq!(auth.access_key_id, "AKIDEXAMPLE");
        assert_eq!(auth.date, "20150830");
        assert_eq!(auth.region, "us-east-1");
        assert_eq!(auth.signed_headers, vec!["host", "x-amz-content-sha256", "x-amz-date"]);
        assert_eq!(auth.signature, "aaaa");
    }

    #[test]
    fn test_sigv4_valid_signature() {
        // Build a request and verify our own signature computation
        let secret = "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY";
        let date = "20130524";
        let region = "us-east-1";

        let key = signing_key(secret, date, region);
        assert!(!key.is_empty());

        let mut headers = BTreeMap::new();
        headers.insert("host".into(), "examplebucket.s3.amazonaws.com".into());
        headers.insert("x-amz-content-sha256".into(), "UNSIGNED-PAYLOAD".into());
        headers.insert("x-amz-date".into(), "20130524T000000Z".into());

        let signed_headers = vec!["host".into(), "x-amz-content-sha256".into(), "x-amz-date".into()];
        let canon = canonical_request("GET", "/test.txt", "", &headers, &signed_headers, "UNSIGNED-PAYLOAD");

        let hash_canon = hex::encode(Sha256::digest(canon.as_bytes()));
        let scope = format!("{}/{}/s3/aws4_request", date, region);
        let string_to_sign = format!("AWS4-HMAC-SHA256\n20130524T000000Z\n{}\n{}", scope, hash_canon);
        let signature = hex::encode(hmac_sha256(&key, string_to_sign.as_bytes()));

        let auth = SigV4Auth {
            access_key_id: "AKIAIOSFODNN7EXAMPLE".into(),
            date: date.into(),
            region: region.into(),
            signed_headers,
            signature,
        };

        let result = verify_signature("GET", "/test.txt", "", &headers, &auth, secret, "UNSIGNED-PAYLOAD");
        assert!(result.is_ok());
    }

    #[test]
    fn test_sigv4_wrong_secret() {
        let mut headers = BTreeMap::new();
        headers.insert("host".into(), "example.com".into());
        headers.insert("x-amz-content-sha256".into(), "UNSIGNED-PAYLOAD".into());
        headers.insert("x-amz-date".into(), "20130524T000000Z".into());

        let auth = SigV4Auth {
            access_key_id: "AKID".into(),
            date: "20130524".into(),
            region: "us-east-1".into(),
            signed_headers: vec!["host".into(), "x-amz-content-sha256".into(), "x-amz-date".into()],
            signature: "invalidsignature".into(),
        };

        let result = verify_signature("GET", "/", "", &headers, &auth, "wrong-secret", "UNSIGNED-PAYLOAD");
        assert!(matches!(result, Err(S3Error::SignatureDoesNotMatch)));
    }

    #[test]
    fn test_sigv4_missing_auth_header() {
        let result = parse_auth_header("Basic abc123");
        assert!(matches!(result, Err(S3Error::AccessDenied)));
    }

    #[test]
    fn test_verify_presigned_signature() {
        let secret = "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY";
        let date = "20130524";
        let region = "us-east-1";
        let amz_date = "20130524T000000Z";

        let mut headers = BTreeMap::new();
        headers.insert("host".into(), "examplebucket.s3.amazonaws.com".into());

        let signed_headers = vec!["host".into()];

        // Build a canonical query that would appear in a presigned URL
        let canonical_query = format!(
            "X-Amz-Algorithm=AWS4-HMAC-SHA256&X-Amz-Credential=AKIAIOSFODNN7EXAMPLE%2F{}%2F{}%2Fs3%2Faws4_request&X-Amz-Date={}&X-Amz-Expires=86400&X-Amz-SignedHeaders=host",
            date, region, amz_date
        );

        // Compute the expected signature
        let canon = canonical_request("GET", "/test.txt", &canonical_query, &headers, &signed_headers, "UNSIGNED-PAYLOAD");
        let hash_canon = hex::encode(Sha256::digest(canon.as_bytes()));
        let scope = format!("{}/{}/s3/aws4_request", date, region);
        let string_to_sign = format!("AWS4-HMAC-SHA256\n{}\n{}\n{}", amz_date, scope, hash_canon);
        let key = signing_key(secret, date, region);
        let signature = hex::encode(hmac_sha256(&key, string_to_sign.as_bytes()));

        let result = verify_presigned_signature(
            "GET", "/test.txt", &canonical_query, &headers, &signed_headers,
            date, amz_date, region, secret, &signature,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_presigned_wrong_signature() {
        let mut headers = BTreeMap::new();
        headers.insert("host".into(), "example.com".into());
        let signed_headers = vec!["host".into()];

        let result = verify_presigned_signature(
            "GET", "/test.txt", "", &headers, &signed_headers,
            "20130524", "20130524T000000Z", "us-east-1", "secret", "invalidsig",
        );
        assert!(matches!(result, Err(S3Error::SignatureDoesNotMatch)));
    }

    #[test]
    fn test_sigv4_unsigned_payload() {
        // Verify UNSIGNED-PAYLOAD is used as the payload hash
        let mut headers = BTreeMap::new();
        headers.insert("host".into(), "bucket.s3.amazonaws.com".into());
        headers.insert("x-amz-content-sha256".into(), "UNSIGNED-PAYLOAD".into());
        headers.insert("x-amz-date".into(), "20230101T000000Z".into());

        let signed_headers = vec!["host".into(), "x-amz-content-sha256".into(), "x-amz-date".into()];
        let canon = canonical_request("PUT", "/key", "", &headers, &signed_headers, "UNSIGNED-PAYLOAD");
        assert!(canon.contains("UNSIGNED-PAYLOAD"));
    }
}
