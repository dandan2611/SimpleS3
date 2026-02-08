# Admin HTTP API

The server exposes a JSON-based admin API under `/_admin/` on a separate port (default `127.0.0.1:9001`).

> **Security:** The admin API runs on its own port, bound to localhost by default. Set `SIMPLES3_ADMIN_TOKEN` to require bearer token authentication. Set `SIMPLES3_ADMIN_ENABLED=false` to disable the admin API entirely.

## Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `SIMPLES3_ADMIN_ENABLED` | `true` | Enable the admin API server (`false` or `0` to disable) |
| `SIMPLES3_ADMIN_BIND` | `127.0.0.1:9001` | Address and port for the admin API |
| `SIMPLES3_ADMIN_TOKEN` | *(none)* | Bearer token required for admin API access (no auth if unset) |

The server binary also accepts `--admin-bind` to override `SIMPLES3_ADMIN_BIND`.

## Endpoints

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/_admin/buckets` | List all buckets |
| `PUT` | `/_admin/buckets/{name}` | Create a bucket |
| `DELETE` | `/_admin/buckets/{name}` | Delete a bucket |
| `PUT` | `/_admin/buckets/{name}/anonymous` | Set anonymous read |
| `GET` | `/_admin/credentials` | List all credentials (secrets masked) |
| `POST` | `/_admin/credentials` | Create a credential |
| `DELETE` | `/_admin/credentials/{access_key_id}` | Revoke a credential |

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
    "anonymous_read": false
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
