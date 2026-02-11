# Lifecycle Policies

Lifecycle policies let you automatically expire (delete) objects based on age, prefix, tags, or a specific date. This is useful for log rotation, temporary file cleanup, and storage cost management.

simples3 implements a subset of the [S3 Lifecycle Configuration API](https://docs.aws.amazon.com/AmazonS3/latest/API/API_PutBucketLifecycleConfiguration.html) using the same XML format.

## How It Works

1. You configure lifecycle rules on a bucket via `PutBucketLifecycleConfiguration`.
2. A background scanner runs periodically (default: every 3600 seconds).
3. On each scan, the server checks all objects matching each rule's prefix and tag filters.
4. Objects that meet the expiration criteria (days-based or date-based) are deleted (both metadata and file data).
5. Deletions are logged at `info` level and counted in the `simples3_lifecycle_expired_total` Prometheus metric.

## Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `SIMPLES3_LIFECYCLE_SCAN_INTERVAL` | `3600` | Interval in seconds between lifecycle expiration scans. Set to `0` to disable the background scanner entirely. |

> **Note:** Disabling the scanner (`0`) does not prevent you from managing lifecycle configurations via the API -- it only stops automatic expiration. You can re-enable scanning by setting a nonzero interval and restarting the server.

## S3 API Operations

All operations use the `?lifecycle` query parameter on the bucket URL.

### PutBucketLifecycleConfiguration

Sets (or replaces) the lifecycle configuration on a bucket. The request body is an XML document.

```
PUT /{bucket}?lifecycle
Content-Type: application/xml
```

**Request body:**

```xml
<?xml version="1.0" encoding="UTF-8"?>
<LifecycleConfiguration>
    <Rule>
        <ID>expire-logs</ID>
        <Filter>
            <Prefix>logs/</Prefix>
        </Filter>
        <Status>Enabled</Status>
        <Expiration>
            <Days>30</Days>
        </Expiration>
    </Rule>
    <Rule>
        <ID>cleanup-tmp</ID>
        <Filter>
            <Prefix>tmp/</Prefix>
        </Filter>
        <Status>Enabled</Status>
        <Expiration>
            <Days>1</Days>
        </Expiration>
    </Rule>
</LifecycleConfiguration>
```

**Response:** `200 OK` on success.

**Validation:**
- Expiration must specify either `Days` (positive integer > 0) or `Date` (ISO 8601), not both.
- `Status` must be `Enabled` or `Disabled`.
- Invalid values return `400 InvalidArgument`.
- The bucket must exist or `404 NoSuchBucket` is returned.

### GetBucketLifecycleConfiguration

Returns the current lifecycle configuration for a bucket.

```
GET /{bucket}?lifecycle
```

**Response:** `200 OK` with `Content-Type: application/xml` and the lifecycle configuration XML.

**Errors:**
- `404 NoSuchLifecycleConfiguration` if no lifecycle configuration has been set on the bucket.
- `404 NoSuchBucket` if the bucket does not exist.

### DeleteBucketLifecycleConfiguration

Removes the lifecycle configuration from a bucket.

```
DELETE /{bucket}?lifecycle
```

**Response:** `204 No Content` on success.

**Errors:**
- `404 NoSuchBucket` if the bucket does not exist.

## XML Format

### Rule Fields

| Element | Parent | Required | Description |
|---------|--------|----------|-------------|
| `Rule` | `LifecycleConfiguration` | yes | Container for a single rule. Multiple rules are supported. |
| `ID` | `Rule` | yes | Unique identifier for the rule (e.g., `"expire-logs"`). |
| `Filter` | `Rule` | yes | Container for filter criteria. |
| `Prefix` | `Filter` | no | Object key prefix to match (e.g., `"logs/"`). Empty string matches all objects. |
| `Tag` | `Filter` | no | Tag filter with `Key` and `Value` children. Object must have this tag to match. |
| `And` | `Filter` | no | Wrapper for combining `Prefix` and one or more `Tag` filters (all must match). |
| `Status` | `Rule` | yes | `Enabled` or `Disabled`. Disabled rules are stored but not evaluated by the scanner. |
| `Expiration` | `Rule` | yes | Container for expiration settings. |
| `Days` | `Expiration` | conditional | Number of days after object creation before the object is deleted. Must be > 0. Mutually exclusive with `Date`. |
| `Date` | `Expiration` | conditional | ISO 8601 date (e.g., `"2025-12-31T00:00:00+00:00"`) at which matching objects expire. Mutually exclusive with `Days`. |

### Filter Combinations

| Filter Type | XML Structure | Description |
|-------------|--------------|-------------|
| Prefix only | `<Filter><Prefix>logs/</Prefix></Filter>` | Match objects by key prefix. |
| Single tag | `<Filter><Tag><Key>env</Key><Value>test</Value></Tag></Filter>` | Match objects with a specific tag. |
| Prefix + tags | `<Filter><And><Prefix>logs/</Prefix><Tag>...</Tag></And></Filter>` | All conditions must match. |
| Multiple tags | `<Filter><And><Tag>...</Tag><Tag>...</Tag></And></Filter>` | All tags must match. |

> **Limitations vs AWS S3:** simples3 supports prefix-based filtering, tag-based filtering, and day-count or date-based expiration. AWS S3 additionally supports transitions between storage classes, noncurrent version expiration, and abort incomplete multipart uploads. These are not implemented.

## Examples

### Tag-Based Filtering

```xml
<?xml version="1.0" encoding="UTF-8"?>
<LifecycleConfiguration>
    <Rule>
        <ID>expire-test-objects</ID>
        <Filter>
            <Tag>
                <Key>env</Key>
                <Value>test</Value>
            </Tag>
        </Filter>
        <Status>Enabled</Status>
        <Expiration>
            <Days>7</Days>
        </Expiration>
    </Rule>
</LifecycleConfiguration>
```

### Combined Prefix + Tag Filter (And)

```xml
<?xml version="1.0" encoding="UTF-8"?>
<LifecycleConfiguration>
    <Rule>
        <ID>expire-staging-logs</ID>
        <Filter>
            <And>
                <Prefix>logs/</Prefix>
                <Tag>
                    <Key>env</Key>
                    <Value>staging</Value>
                </Tag>
            </And>
        </Filter>
        <Status>Enabled</Status>
        <Expiration>
            <Days>14</Days>
        </Expiration>
    </Rule>
</LifecycleConfiguration>
```

### Date-Based Expiration

```xml
<?xml version="1.0" encoding="UTF-8"?>
<LifecycleConfiguration>
    <Rule>
        <ID>expire-at-date</ID>
        <Filter>
            <Prefix>archive/</Prefix>
        </Filter>
        <Status>Enabled</Status>
        <Expiration>
            <Date>2025-12-31T00:00:00+00:00</Date>
        </Expiration>
    </Rule>
</LifecycleConfiguration>
```

### AWS CLI

```bash
# Set a lifecycle rule to expire objects under logs/ after 7 days
aws --endpoint-url http://localhost:9000 s3api put-bucket-lifecycle-configuration \
  --bucket my-bucket \
  --lifecycle-configuration '{
    "Rules": [
      {
        "ID": "expire-logs",
        "Filter": {"Prefix": "logs/"},
        "Status": "Enabled",
        "Expiration": {"Days": 7}
      }
    ]
  }'

# Get the current lifecycle configuration
aws --endpoint-url http://localhost:9000 s3api get-bucket-lifecycle-configuration \
  --bucket my-bucket

# Remove the lifecycle configuration
aws --endpoint-url http://localhost:9000 s3api delete-bucket-lifecycle-configuration \
  --bucket my-bucket
```

### curl

```bash
# Set lifecycle configuration
curl -X PUT "http://localhost:9000/my-bucket?lifecycle" \
  -H "Authorization: ..." \
  -H "Content-Type: application/xml" \
  -d '<?xml version="1.0" encoding="UTF-8"?>
<LifecycleConfiguration>
  <Rule>
    <ID>expire-tmp</ID>
    <Filter><Prefix>tmp/</Prefix></Filter>
    <Status>Enabled</Status>
    <Expiration><Days>1</Days></Expiration>
  </Rule>
</LifecycleConfiguration>'

# Get lifecycle configuration
curl "http://localhost:9000/my-bucket?lifecycle" \
  -H "Authorization: ..."

# Delete lifecycle configuration
curl -X DELETE "http://localhost:9000/my-bucket?lifecycle" \
  -H "Authorization: ..."
```

## Background Scanner Behavior

- The scanner runs in a background tokio task, started alongside the S3 server.
- On the first tick, the scanner does **not** scan (it waits one full interval before the first scan).
- On each scan cycle, it iterates all buckets with lifecycle configurations, then for each enabled rule, lists all objects matching the prefix and deletes those that have expired.
- Both metadata and file data are deleted for expired objects. Associated tags are also cleaned up.
- Errors listing objects or deleting individual objects are logged as warnings and do not abort the scan.
- The scanner respects the `Disabled` status -- disabled rules are skipped entirely.

## Prometheus Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `simples3_lifecycle_expired_total` | Counter | Total number of objects deleted by the lifecycle scanner |
| `simples3_lifecycle_rules_total` | Gauge | Total number of lifecycle rules across all buckets (collected on `/metrics` scrape) |

## Bucket Deletion

When a bucket is deleted via `DeleteBucket`, any associated lifecycle configuration is automatically removed. You do not need to delete the lifecycle configuration before deleting the bucket.
