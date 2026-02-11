use crate::AppState;
use axum::{
    body::Body,
    extract::{Request, State},
    middleware::Next,
    response::{IntoResponse, Response},
};
use http::{HeaderValue, StatusCode};
use std::sync::Arc;

/// Matches an origin against a pattern that may contain a wildcard `*`.
fn origin_matches(pattern: &str, origin: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(idx) = pattern.find('*') {
        let prefix = &pattern[..idx];
        let suffix = &pattern[idx + 1..];
        origin.starts_with(prefix) && origin.ends_with(suffix) && origin.len() >= prefix.len() + suffix.len()
    } else {
        pattern == origin
    }
}

pub async fn cors_middleware(
    State(state): State<Arc<AppState>>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let origin = request
        .headers()
        .get("origin")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let is_preflight = request.method() == http::Method::OPTIONS;

    // Extract bucket name from first path segment
    let path = request.uri().path().trim_start_matches('/');
    let bucket_name = if !path.is_empty() {
        path.split('/').next().map(|s| s.to_string())
    } else {
        None
    };

    // Try to get per-bucket CORS config
    let bucket_cors = bucket_name
        .as_deref()
        .and_then(|b| state.metadata.get_cors_configuration(b).ok());

    let request_method = request
        .headers()
        .get("access-control-request-method")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let request_headers = request
        .headers()
        .get("access-control-request-headers")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    if is_preflight {
        if let Some(ref origin_str) = origin {
            if let Some(ref cors_config) = bucket_cors {
                // Find a matching rule for this origin
                for rule in &cors_config.rules {
                    if rule.allowed_origins.iter().any(|p| origin_matches(p, origin_str)) {
                        let mut response = StatusCode::OK.into_response();
                        let headers = response.headers_mut();

                        // If allowed_origins contains "*", respond with "*", otherwise echo the origin
                        if rule.allowed_origins.iter().any(|o| o == "*") {
                            headers.insert("access-control-allow-origin", HeaderValue::from_static("*"));
                        } else {
                            if let Ok(v) = HeaderValue::from_str(origin_str) {
                                headers.insert("access-control-allow-origin", v);
                            }
                            headers.insert("vary", HeaderValue::from_static("Origin"));
                        }

                        let methods = rule.allowed_methods.join(", ");
                        if let Ok(v) = HeaderValue::from_str(&methods) {
                            headers.insert("access-control-allow-methods", v);
                        }
                        if !rule.allowed_headers.is_empty() {
                            let hdrs = rule.allowed_headers.join(", ");
                            if let Ok(v) = HeaderValue::from_str(&hdrs) {
                                headers.insert("access-control-allow-headers", v);
                            }
                        } else if let Some(ref req_hdrs) = request_headers {
                            // Echo back requested headers if no specific headers configured
                            if let Ok(v) = HeaderValue::from_str(req_hdrs) {
                                headers.insert("access-control-allow-headers", v);
                            }
                        }
                        if !rule.expose_headers.is_empty() {
                            let expose = rule.expose_headers.join(", ");
                            if let Ok(v) = HeaderValue::from_str(&expose) {
                                headers.insert("access-control-expose-headers", v);
                            }
                        }
                        if let Some(max_age) = rule.max_age_seconds {
                            if let Ok(v) = HeaderValue::from_str(&max_age.to_string()) {
                                headers.insert("access-control-max-age", v);
                            }
                        }
                        return response;
                    }
                }
            }

            // Fall back to global CORS config
            return build_global_preflight_response(&state, origin_str, request_method.as_deref(), request_headers.as_deref());
        }

        // No Origin header on preflight — just respond 200
        return StatusCode::OK.into_response();
    }

    // Non-preflight request: run the handler, then add CORS headers
    let mut response = next.run(request).await;

    if let Some(ref origin_str) = origin {
        if let Some(ref cors_config) = bucket_cors {
            for rule in &cors_config.rules {
                if rule.allowed_origins.iter().any(|p| origin_matches(p, origin_str)) {
                    let headers = response.headers_mut();
                    if rule.allowed_origins.iter().any(|o| o == "*") {
                        headers.insert("access-control-allow-origin", HeaderValue::from_static("*"));
                    } else {
                        if let Ok(v) = HeaderValue::from_str(origin_str) {
                            headers.insert("access-control-allow-origin", v);
                        }
                        headers.insert("vary", HeaderValue::from_static("Origin"));
                    }
                    if !rule.expose_headers.is_empty() {
                        let expose = rule.expose_headers.join(", ");
                        if let Ok(v) = HeaderValue::from_str(&expose) {
                            headers.insert("access-control-expose-headers", v);
                        }
                    }
                    return response;
                }
            }
        }

        // Fall back to global CORS
        apply_global_cors_headers(&state, &mut response, origin_str);
    }

    response
}

fn build_global_preflight_response(
    state: &AppState,
    origin: &str,
    _request_method: Option<&str>,
    request_headers: Option<&str>,
) -> Response {
    let mut response = StatusCode::OK.into_response();
    let headers = response.headers_mut();

    match &state.config.cors_origins {
        Some(origins) => {
            if origins.iter().any(|o| origin_matches(o, origin)) {
                if let Ok(v) = HeaderValue::from_str(origin) {
                    headers.insert("access-control-allow-origin", v);
                }
                headers.insert("vary", HeaderValue::from_static("Origin"));
            } else {
                return response;
            }
        }
        None => {
            headers.insert("access-control-allow-origin", HeaderValue::from_static("*"));
        }
    }

    headers.insert("access-control-allow-methods", HeaderValue::from_static("GET, PUT, POST, DELETE, HEAD"));
    if let Some(req_hdrs) = request_headers {
        if let Ok(v) = HeaderValue::from_str(req_hdrs) {
            headers.insert("access-control-allow-headers", v);
        }
    } else {
        headers.insert("access-control-allow-headers", HeaderValue::from_static("*"));
    }
    headers.insert("access-control-expose-headers", HeaderValue::from_static("*"));

    response
}

fn apply_global_cors_headers(state: &AppState, response: &mut Response, origin: &str) {
    let headers = response.headers_mut();
    match &state.config.cors_origins {
        Some(origins) => {
            if origins.iter().any(|o| origin_matches(o, origin)) {
                if let Ok(v) = HeaderValue::from_str(origin) {
                    headers.insert("access-control-allow-origin", v);
                }
                headers.insert("vary", HeaderValue::from_static("Origin"));
                headers.insert("access-control-expose-headers", HeaderValue::from_static("*"));
            }
        }
        None => {
            headers.insert("access-control-allow-origin", HeaderValue::from_static("*"));
            headers.insert("access-control-expose-headers", HeaderValue::from_static("*"));
        }
    }
}
