# Admin HTTP API

The server exposes a JSON-based admin API under `/_admin/` on a separate port (default `127.0.0.1:9001`).

> **Security:** The admin API runs on its own port, bound to localhost by default. Set `SIMPLES3_ADMIN_TOKEN` to require bearer token authentication. Set `SIMPLES3_ADMIN_ENABLED=false` to disable the admin API entirely.

## Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `SIMPLES3_ADMIN_ENABLED` | `true` | Enable the admin API server (`false` or `0` to disable) |
| `SIMPLES3_ADMIN_BIND` | `127.0.0.1:9001` | Address and port for the admin API |
| `SIMPLES3_ADMIN_TOKEN` | *(none)* | Bearer token required for admin API access. **Admin API is denied (401) when no token is configured.** |

The server binary also accepts `--admin-bind` to override `SIMPLES3_ADMIN_BIND`.

## Endpoints

### Admin (authenticated when `SIMPLES3_ADMIN_TOKEN` is set)

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/_admin/buckets` | List all buckets |
| `PUT` | `/_admin/buckets/{name}` | Create a bucket |
| `DELETE` | `/_admin/buckets/{name}` | Delete a bucket |
| `PUT` | `/_admin/buckets/{name}/anonymous` | Set anonymous read |
| `PUT` | `/_admin/buckets/{name}/anonymous-list-public` | Set anonymous list public |
| `GET` | `/_admin/credentials` | List all credentials (secrets masked) |
| `POST` | `/_admin/credentials` | Create a credential |
| `DELETE` | `/_admin/credentials/{access_key_id}` | Revoke a credential |

### Observability (always unauthenticated)

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/health` | Liveness probe -- returns `200 ok` |
| `GET` | `/ready` | Readiness probe -- checks sled and filesystem, returns `200 ready` or `503` |
| `GET` | `/metrics` | Prometheus metrics in text format |

## Bucket Endpoints

### `GET /_admin/buckets`

Returns a JSON array of all buckets.

```bash
curl http://localhost:9001/_admin/buckets
```

```json
[
  {
    "name": "my-bucket",
    "creation_date": "2026-02-08T12:00:00Z",
    "anonymous_read": false,
    "anonymous_list_public": false
  }
]
```

### `PUT /_admin/buckets/{name}`

Creates a new bucket. Returns `201 Created` on success, `409 Conflict` if the bucket already exists.

```bash
curl -X PUT http://localhost:9001/_admin/buckets/my-bucket
```

### `DELETE /_admin/buckets/{name}`

Deletes an empty bucket. Returns `204 No Content` on success, `409 Conflict` if the bucket is not empty, `404 Not Found` if it does not exist.

```bash
curl -X DELETE http://localhost:9001/_admin/buckets/my-bucket
```

### `PUT /_admin/buckets/{name}/anonymous`

Enables or disables anonymous read access on a bucket. Accepts a JSON body with an `enabled` boolean field. Returns `200 OK` on success.

```bash
# Enable anonymous read
curl -X PUT http://localhost:9001/_admin/buckets/my-bucket/anonymous \
  -H "Content-Type: application/json" \
  -d '{"enabled": true}'

# Disable anonymous read
curl -X PUT http://localhost:9001/_admin/buckets/my-bucket/anonymous \
  -H "Content-Type: application/json" \
  -d '{"enabled": false}'
```

### `PUT /_admin/buckets/{name}/anonymous-list-public`

Enables or disables anonymous listing of public objects on a bucket. When enabled, unauthenticated `ListObjectsV2` requests are allowed but results are filtered to only include objects with `public: true`. Accepts a JSON body with an `enabled` boolean field.

```bash
# Enable anonymous list of public objects
curl -X PUT http://localhost:9001/_admin/buckets/my-bucket/anonymous-list-public \
  -H "Content-Type: application/json" \
  -d '{"enabled": true}'

# Disable anonymous list of public objects
curl -X PUT http://localhost:9001/_admin/buckets/my-bucket/anonymous-list-public \
  -H "Content-Type: application/json" \
  -d '{"enabled": false}'
```

## Credential Endpoints

### `POST /_admin/credentials`

Creates a new access key pair. Accepts an optional `description` field. Returns `201 Created` with the full credential including the secret — **this is the only time the secret is returned**.

```bash
curl -X POST http://localhost:9001/_admin/credentials \
  -H "Content-Type: application/json" \
  -d '{"description": "CI pipeline key"}'
```

```json
{
  "access_key_id": "AKXXXXXXXXXXXXXXXX",
  "secret_access_key": "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX",
  "description": "CI pipeline key",
  "created": "2026-02-08T12:00:00Z",
  "active": true
}
```

### `GET /_admin/credentials`

Returns a JSON array of all credentials. Secret access keys are masked with `********`.

```bash
curl http://localhost:9001/_admin/credentials
```

```json
[
  {
    "access_key_id": "AKXXXXXXXXXXXXXXXX",
    "secret_access_key": "********",
    "description": "CI pipeline key",
    "created": "2026-02-08T12:00:00Z",
    "active": true
  }
]
```

### `DELETE /_admin/credentials/{access_key_id}`

Revokes a credential (deactivates it without deleting). Returns `200 OK` on success. Revoked credentials will be rejected by the S3 authentication middleware.

```bash
curl -X DELETE http://localhost:9001/_admin/credentials/AKXXXXXXXXXXXXXXXX
```

## Health Checks & Metrics

The admin port also serves unauthenticated observability endpoints for use with Kubernetes probes and Prometheus scrapers.

### `GET /health`

Returns `200 ok`. Pure liveness check with no dependency verification.

```bash
curl http://localhost:9001/health
# ok
```

### `GET /ready`

Verifies that the metadata store (sled) is accessible and the data directory is writable. Returns `200 ready` on success or `503 Service Unavailable` with an error description on failure.

```bash
curl http://localhost:9001/ready
# ready
```

### `GET /metrics`

Returns Prometheus-format metrics. Storage gauges are collected on-demand at scrape time.

```bash
curl http://localhost:9001/metrics
```

**Request metrics** (recorded per S3 request by middleware):

| Metric | Type | Labels |
|--------|------|--------|
| `s3_requests_total` | Counter | `operation` |
| `s3_request_duration_seconds` | Histogram | `operation` |
| `s3_errors_total` | Counter | `status` |

**Storage metrics** (collected on scrape):

| Metric | Type | Description |
|--------|------|-------------|
| `simples3_bucket_count` | Gauge | Number of buckets |
| `simples3_total_object_count` | Gauge | Total objects across all buckets |
| `simples3_total_storage_bytes` | Gauge | Total storage bytes across all buckets |
| `simples3_credential_count` | Gauge | Number of credentials |
| `simples3_active_multipart_uploads` | Gauge | Active multipart uploads |
| `simples3_lifecycle_rules_total` | Gauge | Total lifecycle rules across all buckets |
| `simples3_uptime_seconds` | Gauge | Server uptime in seconds |

**Background task metrics** (recorded by background workers):

| Metric | Type | Description |
|--------|------|-------------|
| `simples3_multipart_expired_total` | Counter | Multipart uploads cleaned up by the background task |
| `simples3_lifecycle_expired_total` | Counter | Objects deleted by the lifecycle expiration scanner |

## Bootstrap / Init Config

Instead of manually creating buckets and credentials via CLI or API, you can provide a TOML init config file that the server reads on boot. This is useful for Docker, CI, and automated deployments.

Set the path via `--init-config` flag or `SIMPLES3_INIT_CONFIG` env var. If unset, no init file is loaded.

### Format

```toml
[[buckets]]
name = "my-bucket"

[[buckets]]
name = "public-assets"
anonymous_read = true

[[buckets]]
name = "mixed-access"
anonymous_list_public = true

[[buckets]]
name = "web-app"
cors_origins = ["https://example.com", "https://app.example.com"]

[[credentials]]
access_key_id = "AKID_CI_PIPELINE"
secret_access_key = "supersecretkey123"
description = "CI pipeline"

[[credentials]]
access_key_id = "AKID_DEV"
secret_access_key = "devkey456"
description = "Development"
```

### Fields

**`[[buckets]]`**

| Field | Required | Default | Description |
|-------|----------|---------|-------------|
| `name` | yes | | Bucket name |
| `anonymous_read` | no | `false` | Enable anonymous read access on this bucket |
| `anonymous_list_public` | no | `false` | Allow anonymous users to list public objects only |
| `cors_origins` | no | *(none)* | List of allowed CORS origins (creates a CORS config with all methods/headers) |

**`[[credentials]]`**

| Field | Required | Default | Description |
|-------|----------|---------|-------------|
| `access_key_id` | yes | | Access key ID |
| `secret_access_key` | yes | | Secret access key |
| `description` | no | `""` | Human-readable description |

### Behavior

- The init file is applied **before** the server starts accepting requests.
- It is **idempotent**: if a bucket or credential already exists, it is silently skipped. This makes it safe to use on every boot.
- Buckets that already exist but have a different `anonymous_read` setting will be updated to match the config.
- Created items are logged at `info` level; skipped items at `debug` level.

## CLI Reference

The CLI has two modes:

- **Online (default)**: communicates with a running server via the `/_admin/` HTTP API. Use `--server-url` to set the admin API address (default: `http://localhost:9001`). Use `--admin-token` to authenticate (or set `SIMPLES3_ADMIN_TOKEN`).
- **Offline** (`--offline`): operates directly on the sled metadata database. Only works when the server is **not** running (sled uses exclusive file locks). Use `--metadata-dir` to point at a custom metadata directory.

### Bucket Management

```bash
# Create a bucket (online, talking to running server)
simples3-cli bucket create <name>

# Create a bucket (offline, server must be stopped)
simples3-cli --offline bucket create <name>

# List all buckets
simples3-cli bucket list

# Delete an empty bucket
simples3-cli bucket delete <name>

# Enable anonymous read access on a bucket
simples3-cli bucket config <name> anonymous true

# Disable anonymous read access
simples3-cli bucket config <name> anonymous false
```

### Credential Management

```bash
# Create a new access key pair
simples3-cli credentials create --description "my key"

# List all credentials
simples3-cli credentials list

# Revoke a credential (deactivates it, does not delete)
simples3-cli credentials revoke <access-key-id>
```

### CLI Flags

| Flag | Default | Description |
|------|---------|-------------|
| `--server-url` | `http://localhost:9001` | Admin API URL for online mode (env: `SIMPLES3_ADMIN_URL`) |
| `--admin-token` | *(none)* | Bearer token for admin API authentication (env: `SIMPLES3_ADMIN_TOKEN`) |
| `--offline` | `false` | Use direct sled access instead of HTTP |
| `--metadata-dir` | from `SIMPLES3_METADATA_DIR` | Metadata directory (offline mode only) |
