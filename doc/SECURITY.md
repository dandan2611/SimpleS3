# Security Hardening

This document describes the security measures implemented in simples3.

## Admin API Authentication

The admin API (`/_admin/` endpoints) requires a bearer token when `SIMPLES3_ADMIN_TOKEN` is configured. When no token is set, all admin API requests are **denied with 401 Unauthorized** -- the admin API is not left open by default.

Set the token via environment variable:

```bash
SIMPLES3_ADMIN_TOKEN=my-secret-token
```

The token comparison uses SHA-256 hashing followed by constant-time comparison to prevent timing attacks and length leaks.

## Bucket Name Validation

Bucket names are validated against S3 naming rules:

- Must be 3-63 characters long
- Only lowercase letters, numbers, hyphens, and periods
- Cannot start or end with a hyphen or period
- Cannot contain consecutive periods (`..`)

## Request Body Size Limits

All request bodies are limited to prevent memory exhaustion:

| Limit | Default | Env Var |
|-------|---------|---------|
| Object/part upload | 5 GiB | `SIMPLES3_MAX_OBJECT_SIZE` |
| XML bodies (lifecycle, CORS, tagging, etc.) | 256 KiB | `SIMPLES3_MAX_XML_BODY_SIZE` |
| Bucket policy JSON | 20 KiB | `SIMPLES3_MAX_POLICY_BODY_SIZE` |

## Path Traversal Protection

The filesystem storage layer validates all object keys and normalizes paths to prevent directory traversal attacks. Keys containing `..` or absolute paths are rejected.

## Constant-Time Signature Comparison

AWS Signature V4 verification uses constant-time comparison for signature matching, preventing timing side-channel attacks.

## Error Message Sanitization

Internal server errors (database failures, filesystem errors, etc.) are logged server-side with full details but return a generic "Internal server error" message to clients. This prevents leaking internal implementation details.

## CORS Configuration

CORS can be configured globally via `SIMPLES3_CORS_ORIGINS` or per-bucket via the S3 CORS XML API. See [CORS.md](CORS.md) for details. Per-bucket CORS rules are applied dynamically and take precedence over global configuration.
