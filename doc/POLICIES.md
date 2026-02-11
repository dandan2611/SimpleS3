# Bucket Policies

Bucket policies provide fine-grained access control using a JSON format compatible with the [AWS IAM policy language](https://docs.aws.amazon.com/AmazonS3/latest/userguide/bucket-policies.html). You can use policies to grant anonymous access to specific actions, restrict authenticated users from certain operations, or define resource-level permissions.

## How It Works

Bucket policies are evaluated in the S3 authentication middleware on every request:

- **Anonymous requests** (no `Authorization` header): After the existing anonymous access checks (global anonymous, per-bucket anonymous read, per-object public), the policy is evaluated. An explicit `Allow` grants access. An explicit `Deny` blocks access. An implicit deny (no matching statement) falls through to the default behavior (deny).

- **Authenticated requests** (valid SigV4 signature): After successful signature verification, the policy is evaluated. An explicit `Deny` blocks the request even though the user is authenticated. `Allow` and implicit deny do not change the outcome -- authenticated users are already permitted by their credentials.

This means:
- Policies can **grant** anonymous access that would otherwise be denied.
- Policies can **deny** access for authenticated users that would otherwise be allowed.
- Explicit `Deny` always wins over `Allow` (same as AWS).

## S3 API Operations

All operations use the `?policy` query parameter on the bucket URL.

### PutBucketPolicy

Sets (or replaces) the bucket policy. The request body is a JSON document.

```
PUT /{bucket}?policy
Content-Type: application/json
```

**Request body:**

```json
{
    "Version": "2012-10-17",
    "Statement": [
        {
            "Sid": "AllowPublicRead",
            "Effect": "Allow",
            "Principal": "*",
            "Action": "s3:GetObject",
            "Resource": "arn:aws:s3:::my-bucket/*"
        }
    ]
}
```

**Response:** `204 No Content` on success.

**Validation:**
- The body must be valid JSON conforming to the policy schema.
- At least one statement is required.
- Invalid JSON or empty statement arrays return `400 InvalidArgument`.
- The bucket must exist or `404 NoSuchBucket` is returned.

### GetBucketPolicy

Returns the current bucket policy as JSON.

```
GET /{bucket}?policy
```

**Response:** `200 OK` with `Content-Type: application/json` and the policy document.

**Errors:**
- `404 NoSuchBucketPolicy` if no policy has been set on the bucket.
- `404 NoSuchBucket` if the bucket does not exist.

### DeleteBucketPolicy

Removes the bucket policy.

```
DELETE /{bucket}?policy
```

**Response:** `204 No Content` on success.

**Errors:**
- `404 NoSuchBucket` if the bucket does not exist.

## Policy Document Format

### Top-Level Fields

| Field | Required | Description |
|-------|----------|-------------|
| `Version` | yes | Policy version string. Use `"2012-10-17"`. |
| `Statement` | yes | Array of one or more policy statements. |

### Statement Fields

| Field | Required | Type | Description |
|-------|----------|------|-------------|
| `Sid` | no | String | Optional statement identifier for documentation purposes. |
| `Effect` | yes | `"Allow"` or `"Deny"` | Whether this statement allows or denies the matched actions. |
| `Principal` | yes | String or Object | Who the statement applies to (see [Principals](#principals)). |
| `Action` | yes | String or Array | S3 action(s) the statement applies to (see [Actions](#actions)). |
| `Resource` | yes | String or Array | ARN(s) of the resources the statement applies to (see [Resources](#resources)). |
| `Condition` | no | Object | Condition block for fine-grained access control (see [Conditions](#conditions)). |

### Principals

The `Principal` field specifies who the policy applies to.

| Format | Example | Description |
|--------|---------|-------------|
| Wildcard | `"*"` | Matches all principals (anonymous and authenticated). |
| Mapped | `{"AWS": "AKID123"}` | Matches a specific access key ID. |
| Mapped array | `{"AWS": ["AKID1", "AKID2"]}` | Matches any of the listed access key IDs. |
| Mapped wildcard | `{"AWS": "*"}` | Matches any authenticated user. |

When a request is anonymous (no auth header), the principal ID is `null`. Only wildcard principals (`"*"`) match anonymous requests. Mapped principals only match authenticated users by their access key ID.

### Actions

The `Action` field specifies which S3 operations the statement applies to.

| Format | Example | Description |
|--------|---------|-------------|
| Exact | `"s3:GetObject"` | Matches a single action. |
| Wildcard | `"s3:*"` or `"*"` | Matches all S3 actions. |
| Prefix wildcard | `"s3:Get*"` | Matches all actions starting with the prefix. |
| Array | `["s3:GetObject", "s3:HeadObject"]` | Matches any of the listed actions. |

**Supported S3 actions:**

| Action | Operations |
|--------|-----------|
| `s3:GetObject` | `GetObject` |
| `s3:HeadObject` | `HeadObject` |
| `s3:PutObject` | `PutObject`, `CreateMultipartUpload`, `UploadPart`, `CompleteMultipartUpload` |
| `s3:DeleteObject` | `DeleteObject`, `DeleteObjects` |
| `s3:ListBucket` | `ListObjectsV2` |
| `s3:ListAllMyBuckets` | `ListBuckets` |
| `s3:CreateBucket` | `CreateBucket` |
| `s3:DeleteBucket` | `DeleteBucket` |
| `s3:HeadBucket` | `HeadBucket` |
| `s3:PutObjectTagging` | `PutObjectTagging` |
| `s3:GetObjectTagging` | `GetObjectTagging` |
| `s3:DeleteObjectTagging` | `DeleteObjectTagging` |
| `s3:PutObjectAcl` | `PutObjectAcl` |
| `s3:GetObjectAcl` | `GetObjectAcl` |
| `s3:AbortMultipartUpload` | `AbortMultipartUpload` |
| `s3:ListMultipartUploadParts` | `ListParts` |
| `s3:PutLifecycleConfiguration` | `PutBucketLifecycleConfiguration`, `DeleteBucketLifecycleConfiguration` |
| `s3:GetLifecycleConfiguration` | `GetBucketLifecycleConfiguration` |
| `s3:PutBucketPolicy` | `PutBucketPolicy` |
| `s3:GetBucketPolicy` | `GetBucketPolicy` |
| `s3:DeleteBucketPolicy` | `DeleteBucketPolicy` |

### Resources

The `Resource` field specifies which bucket or objects the statement applies to, using ARN format.

| Format | Example | Description |
|--------|---------|-------------|
| Bucket ARN | `"arn:aws:s3:::my-bucket"` | Matches the bucket itself (for bucket-level operations). |
| Object ARN | `"arn:aws:s3:::my-bucket/file.txt"` | Matches a specific object. |
| Object wildcard | `"arn:aws:s3:::my-bucket/*"` | Matches all objects in the bucket. |
| Prefix wildcard | `"arn:aws:s3:::my-bucket/logs/*"` | Matches all objects under the `logs/` prefix. |
| Global wildcard | `"*"` | Matches all resources. |

## Evaluation Logic

Policy evaluation follows the standard AWS model:

1. Iterate all statements in the policy.
2. For each statement, check if the principal, action, and resource all match the current request.
3. If a matching statement has `Effect: Deny`, return **Explicit Deny** immediately (short-circuit).
4. If a matching statement has `Effect: Allow`, record it.
5. After all statements are evaluated:
   - If any `Allow` was found, return **Explicit Allow**.
   - Otherwise, return **Implicit Deny** (no matching statement).

**Key rule: Deny always wins.** If both an Allow and a Deny statement match the same request, the Deny takes precedence.

### Interaction with Other Access Controls

The policy evaluation interacts with simples3's other access control mechanisms in this order:

1. **Presigned URLs** -- checked first, bypass all other auth.
2. **Global anonymous mode** (`SIMPLES3_ANONYMOUS_GLOBAL=true`) -- bypasses all auth.
3. **Per-bucket anonymous read** -- allows read-only operations on enabled buckets.
4. **Per-object public access** -- allows GET/HEAD on public objects.
5. **Bucket policy (anonymous)** -- evaluated for anonymous requests that weren't already allowed. Explicit Allow grants access. Explicit Deny blocks. Implicit Deny falls through to the default denial.
6. **SigV4 authentication** -- standard credential-based auth.
7. **Bucket policy (authenticated)** -- evaluated after successful SigV4. Only Explicit Deny has an effect (blocks the request). Allow and Implicit Deny do not change the outcome.

## Examples

### Allow Anonymous Read Access

Grant anonymous users the ability to read objects from a bucket:

```json
{
    "Version": "2012-10-17",
    "Statement": [
        {
            "Sid": "PublicRead",
            "Effect": "Allow",
            "Principal": "*",
            "Action": ["s3:GetObject", "s3:HeadObject"],
            "Resource": "arn:aws:s3:::my-bucket/*"
        }
    ]
}
```

### Allow Anonymous Read, Deny a Specific Path

Allow public reads but block access to a sensitive prefix:

```json
{
    "Version": "2012-10-17",
    "Statement": [
        {
            "Sid": "PublicRead",
            "Effect": "Allow",
            "Principal": "*",
            "Action": "s3:GetObject",
            "Resource": "arn:aws:s3:::my-bucket/*"
        },
        {
            "Sid": "DenyPrivate",
            "Effect": "Deny",
            "Principal": "*",
            "Action": "s3:GetObject",
            "Resource": "arn:aws:s3:::my-bucket/private/*"
        }
    ]
}
```

### Restrict a Specific User

Deny a specific access key from deleting objects:

```json
{
    "Version": "2012-10-17",
    "Statement": [
        {
            "Sid": "DenyDeleteForReadOnlyUser",
            "Effect": "Deny",
            "Principal": {"AWS": "AKID_READONLY"},
            "Action": "s3:DeleteObject",
            "Resource": "arn:aws:s3:::my-bucket/*"
        }
    ]
}
```

### Allow All Actions for a Specific User

```json
{
    "Version": "2012-10-17",
    "Statement": [
        {
            "Sid": "FullAccess",
            "Effect": "Allow",
            "Principal": {"AWS": "AKID_ADMIN"},
            "Action": "s3:*",
            "Resource": "*"
        }
    ]
}
```

### AWS CLI

```bash
# Set a bucket policy
aws --endpoint-url http://localhost:9000 s3api put-bucket-policy \
  --bucket my-bucket \
  --policy '{
    "Version": "2012-10-17",
    "Statement": [
      {
        "Sid": "PublicRead",
        "Effect": "Allow",
        "Principal": "*",
        "Action": "s3:GetObject",
        "Resource": "arn:aws:s3:::my-bucket/*"
      }
    ]
  }'

# Get the current bucket policy
aws --endpoint-url http://localhost:9000 s3api get-bucket-policy \
  --bucket my-bucket

# Delete the bucket policy
aws --endpoint-url http://localhost:9000 s3api delete-bucket-policy \
  --bucket my-bucket
```

### curl

```bash
# Set bucket policy
curl -X PUT "http://localhost:9000/my-bucket?policy" \
  -H "Authorization: ..." \
  -H "Content-Type: application/json" \
  -d '{
    "Version": "2012-10-17",
    "Statement": [{
      "Effect": "Allow",
      "Principal": "*",
      "Action": "s3:GetObject",
      "Resource": "arn:aws:s3:::my-bucket/*"
    }]
  }'

# Get bucket policy
curl "http://localhost:9000/my-bucket?policy" \
  -H "Authorization: ..."

# Delete bucket policy
curl -X DELETE "http://localhost:9000/my-bucket?policy" \
  -H "Authorization: ..."
```

## Conditions

Conditions allow fine-grained control by evaluating request properties. A condition block contains one or more operator blocks. All operator blocks must match for the statement to apply (AND logic). Within each operator block, multiple values for a key are OR'd.

### Supported Operators

| Operator | Description |
|----------|-------------|
| `StringEquals` | Exact string match. |
| `StringNotEquals` | Negated exact string match. |
| `StringLike` | Glob-style pattern match (`*` matches any sequence, `?` matches single char). |
| `StringNotLike` | Negated glob-style pattern match. |
| `IpAddress` | Source IP matches a CIDR block (e.g., `10.0.0.0/8`). |
| `NotIpAddress` | Source IP does not match a CIDR block. |
| `DateGreaterThan` | Current time is after the specified ISO 8601 date. |
| `DateLessThan` | Current time is before the specified ISO 8601 date. |
| `Bool` | Boolean comparison (e.g., `"true"` or `"false"`). |

### Supported Condition Keys

| Key | Type | Description |
|-----|------|-------------|
| `aws:SourceIp` | IP/CIDR | The IP address of the requester. Used with `IpAddress`/`NotIpAddress`. |
| `aws:CurrentTime` | Date | The current server time. Used with `DateGreaterThan`/`DateLessThan`. |
| `aws:SecureTransport` | Bool | Whether the request was made over HTTPS (checks `X-Forwarded-Proto` header or URI scheme). |
| `s3:prefix` | String | The `prefix` query parameter from ListObjectsV2 requests. |

### Condition Examples

**Allow access only from a specific IP range:**

```json
{
    "Version": "2012-10-17",
    "Statement": [
        {
            "Sid": "AllowFromOffice",
            "Effect": "Allow",
            "Principal": "*",
            "Action": "s3:GetObject",
            "Resource": "arn:aws:s3:::my-bucket/*",
            "Condition": {
                "IpAddress": {
                    "aws:SourceIp": "10.0.0.0/8"
                }
            }
        }
    ]
}
```

**Deny access after a specific date:**

```json
{
    "Version": "2012-10-17",
    "Statement": [
        {
            "Sid": "DenyAfterExpiry",
            "Effect": "Deny",
            "Principal": "*",
            "Action": "s3:*",
            "Resource": "arn:aws:s3:::my-bucket/*",
            "Condition": {
                "DateGreaterThan": {
                    "aws:CurrentTime": "2025-12-31T00:00:00+00:00"
                }
            }
        }
    ]
}
```

**Require HTTPS:**

```json
{
    "Version": "2012-10-17",
    "Statement": [
        {
            "Sid": "DenyInsecure",
            "Effect": "Deny",
            "Principal": "*",
            "Action": "s3:*",
            "Resource": "*",
            "Condition": {
                "Bool": {
                    "aws:SecureTransport": "false"
                }
            }
        }
    ]
}
```

### Condition Evaluation Rules

- If a statement has a `Condition` block and a `RequestContext` is available, the conditions are evaluated. The statement only applies if all conditions match.
- If a statement has a `Condition` block but no `RequestContext` is available (e.g., internal calls), the statement is skipped (conservative: cannot evaluate means do not apply).
- Statements without `Condition` blocks are evaluated normally regardless of context.

## Limitations vs AWS S3

- **Cross-account principals** (e.g., `arn:aws:iam::123456789012:root`) are not supported. Principal matching uses access key IDs directly.
- **NotPrincipal**, **NotAction**, and **NotResource** fields are not supported.
- **Policy size limits** are not enforced.
- **Policy variables** (e.g., `${aws:username}`) are not substituted.
- **Condition operators** beyond those listed above (e.g., `ArnLike`, `NumericEquals`) are not supported.

## Bucket Deletion

When a bucket is deleted via `DeleteBucket`, any associated bucket policy is automatically removed. You do not need to delete the policy before deleting the bucket.
