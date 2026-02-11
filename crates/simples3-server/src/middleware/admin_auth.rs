use crate::AppState;
use axum::{
    body::Body,
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::sync::Arc;

pub async fn admin_auth_middleware(
    State(state): State<Arc<AppState>>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let expected_token = match &state.config.admin_token {
        Some(token) => token,
        None => {
            tracing::warn!("Admin request rejected: SIMPLES3_ADMIN_TOKEN is not configured");
            return (
                StatusCode::UNAUTHORIZED,
                axum::Json(serde_json::json!({ "error": "Admin token not configured" })),
            )
                .into_response();
        }
    };

    let provided = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    match provided {
        Some(token) if constant_time_eq(token.as_bytes(), expected_token.as_bytes()) => {
            next.run(request).await
        }
        _ => (
            StatusCode::UNAUTHORIZED,
            axum::Json(serde_json::json!({ "error": "Unauthorized" })),
        )
            .into_response(),
    }
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    use sha2::{Digest, Sha256};
    // Hash both inputs before comparison so length differences
    // don't leak timing information about the expected token.
    let hash_a = Sha256::digest(a);
    let hash_b = Sha256::digest(b);
    let mut diff = 0u8;
    for (x, y) in hash_a.iter().zip(hash_b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}
