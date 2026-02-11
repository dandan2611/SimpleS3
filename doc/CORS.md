# CORS Configuration

simples3 supports both global and per-bucket CORS (Cross-Origin Resource Sharing) configuration for browser-based access to S3 resources.

## Global CORS

Set the `SIMPLES3_CORS_ORIGINS` environment variable to a comma-separated list of allowed origins. If unset, all origins are allowed by default.

```bash
# Allow specific origins
SIMPLES3_CORS_ORIGINS=https://example.com,https://app.example.com

# Allow all origins (default when unset)
```

Global CORS is used as a fallback when a bucket has no per-bucket CORS configuration.

## Per-Bucket CORS (S3 XML API)

Per-bucket CORS overrides the global configuration for that bucket. Manage it using the standard S3 CORS API:

### PutBucketCors

```bash
aws --endpoint-url http://localhost:9000 s3api put-bucket-cors \
  --bucket my-bucket \
  --cors-configuration '{
    "CORSRules": [
      {
        "AllowedOrigins": ["https://example.com"],
        "AllowedMethods": ["GET", "PUT"],
        "AllowedHeaders": ["*"],
        "ExposeHeaders": ["x-amz-request-id"],
        "MaxAgeSeconds": 3600
      }
    ]
  }'
```

### GetBucketCors

```bash
aws --endpoint-url http://localhost:9000 s3api get-bucket-cors --bucket my-bucket
```

### DeleteBucketCors

```bash
aws --endpoint-url http://localhost:9000 s3api delete-bucket-cors --bucket my-bucket
```

## XML Format

```xml
<CORSConfiguration xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
  <CORSRule>
    <ID>rule-1</ID>
    <AllowedOrigin>https://example.com</AllowedOrigin>
    <AllowedOrigin>https://app.example.com</AllowedOrigin>
    <AllowedMethod>GET</AllowedMethod>
    <AllowedMethod>PUT</AllowedMethod>
    <AllowedHeader>*</AllowedHeader>
    <ExposeHeader>x-amz-request-id</ExposeHeader>
    <MaxAgeSeconds>3600</MaxAgeSeconds>
  </CORSRule>
</CORSConfiguration>
```

Each `CORSRule` must have at least one `AllowedOrigin` and one `AllowedMethod`. Other elements are optional.

| Element | Required | Description |
|---------|----------|-------------|
| `ID` | no | Optional rule identifier |
| `AllowedOrigin` | yes | Origin(s) allowed (`*` for all, or specific URLs) |
| `AllowedMethod` | yes | HTTP method(s) allowed (GET, PUT, POST, DELETE, HEAD) |
| `AllowedHeader` | no | Request headers allowed in preflight (`*` for all) |
| `ExposeHeader` | no | Response headers exposed to the browser |
| `MaxAgeSeconds` | no | Preflight cache duration in seconds |

## Init Config Support

You can configure per-bucket CORS in the init config TOML file:

```toml
[[buckets]]
name = "web-app"
cors_origins = ["https://example.com", "https://app.example.com"]
```

This creates a CORS configuration with the specified origins, all HTTP methods, and all headers allowed. For more granular control, use the S3 XML API after startup.

## Precedence

1. **Per-bucket CORS** (set via `PutBucketCors` or init config) takes priority
2. **Global CORS** (`SIMPLES3_CORS_ORIGINS` env var) is used as fallback
3. If neither is configured, all origins are allowed

## Wildcard Support

Origin patterns support `*` as a wildcard:
- `*` matches any origin
- `https://*.example.com` matches any subdomain of example.com
