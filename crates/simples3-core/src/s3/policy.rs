use crate::s3::types::{BucketPolicy, OneOrMany, PolicyCondition, PolicyEffect, PolicyPrincipal};
use chrono::{DateTime, Utc};
use std::net::IpAddr;

#[derive(Debug, Clone, PartialEq)]
pub enum PolicyDecision {
    ExplicitAllow,
    ExplicitDeny,
    ImplicitDeny,
}

pub fn operation_to_s3_action(op_name: &str) -> &str {
    match op_name {
        "ListBuckets" => "s3:ListAllMyBuckets",
        "CreateBucket" => "s3:CreateBucket",
        "DeleteBucket" => "s3:DeleteBucket",
        "HeadBucket" => "s3:HeadBucket",
        "ListObjectsV2" => "s3:ListBucket",
        "PutObject" => "s3:PutObject",
        "GetObject" => "s3:GetObject",
        "HeadObject" => "s3:HeadObject",
        "DeleteObject" => "s3:DeleteObject",
        "DeleteObjects" => "s3:DeleteObject",
        "PutObjectTagging" => "s3:PutObjectTagging",
        "GetObjectTagging" => "s3:GetObjectTagging",
        "DeleteObjectTagging" => "s3:DeleteObjectTagging",
        "PutObjectAcl" => "s3:PutObjectAcl",
        "GetObjectAcl" => "s3:GetObjectAcl",
        "CreateMultipartUpload" => "s3:PutObject",
        "UploadPart" => "s3:PutObject",
        "CompleteMultipartUpload" => "s3:PutObject",
        "AbortMultipartUpload" => "s3:AbortMultipartUpload",
        "ListParts" => "s3:ListMultipartUploadParts",
        "PutBucketLifecycleConfiguration" => "s3:PutLifecycleConfiguration",
        "GetBucketLifecycleConfiguration" => "s3:GetLifecycleConfiguration",
        "DeleteBucketLifecycleConfiguration" => "s3:PutLifecycleConfiguration",
        "PutBucketPolicy" => "s3:PutBucketPolicy",
        "GetBucketPolicy" => "s3:GetBucketPolicy",
        "DeleteBucketPolicy" => "s3:DeleteBucketPolicy",
        other => {
            // Fallback: return s3:<op_name>
            // This leaks the op_name which is fine for unknown operations
            Box::leak(format!("s3:{}", other).into_boxed_str())
        }
    }
}

pub struct RequestContext {
    pub source_ip: Option<IpAddr>,
    pub current_time: DateTime<Utc>,
    pub secure_transport: bool,
    pub s3_prefix: Option<String>,
}

pub fn evaluate_policy(
    policy: &BucketPolicy,
    s3_action: &str,
    bucket: &str,
    key: Option<&str>,
    principal_id: Option<&str>,
    context: Option<&RequestContext>,
) -> PolicyDecision {
    let mut has_allow = false;

    for statement in &policy.statements {
        if !principal_matches(&statement.principal, principal_id) {
            continue;
        }
        if !action_matches(&statement.action, s3_action) {
            continue;
        }
        if !resource_matches(&statement.resource, bucket, key) {
            continue;
        }

        // Evaluate conditions if present
        if let Some(ref condition) = statement.condition {
            match context {
                Some(ctx) => {
                    if !evaluate_conditions(condition, ctx) {
                        continue;
                    }
                }
                None => {
                    // No context available — skip statements with conditions (conservative)
                    continue;
                }
            }
        }

        match statement.effect {
            PolicyEffect::Deny => return PolicyDecision::ExplicitDeny,
            PolicyEffect::Allow => has_allow = true,
        }
    }

    if has_allow {
        PolicyDecision::ExplicitAllow
    } else {
        PolicyDecision::ImplicitDeny
    }
}

fn evaluate_conditions(condition: &PolicyCondition, ctx: &RequestContext) -> bool {
    // All operator blocks must match (AND between operators)
    for (operator, key_values) in condition {
        for (cond_key, cond_values) in key_values {
            let values: Vec<&str> = cond_values.as_slice().iter().map(|s| s.as_str()).collect();
            let matched = match operator.as_str() {
                "StringEquals" => eval_string_equals(cond_key, &values, ctx),
                "StringNotEquals" => !eval_string_equals(cond_key, &values, ctx),
                "StringLike" => eval_string_like(cond_key, &values, ctx),
                "StringNotLike" => !eval_string_like(cond_key, &values, ctx),
                "IpAddress" => eval_ip_address(cond_key, &values, ctx),
                "NotIpAddress" => !eval_ip_address(cond_key, &values, ctx),
                "DateGreaterThan" => eval_date_greater_than(cond_key, &values, ctx),
                "DateLessThan" => eval_date_less_than(cond_key, &values, ctx),
                "Bool" => eval_bool(cond_key, &values, ctx),
                _ => false, // Unknown operator: condition fails
            };
            if !matched {
                return false;
            }
        }
    }
    true
}

fn resolve_condition_key(cond_key: &str, ctx: &RequestContext) -> Option<String> {
    match cond_key {
        "aws:SourceIp" => ctx.source_ip.map(|ip| ip.to_string()),
        "aws:CurrentTime" => Some(ctx.current_time.to_rfc3339()),
        "aws:SecureTransport" => Some(ctx.secure_transport.to_string()),
        "s3:prefix" => ctx.s3_prefix.clone(),
        _ => None,
    }
}

fn eval_string_equals(cond_key: &str, values: &[&str], ctx: &RequestContext) -> bool {
    if let Some(actual) = resolve_condition_key(cond_key, ctx) {
        // OR within values
        values.iter().any(|v| *v == actual)
    } else {
        false
    }
}

fn eval_string_like(cond_key: &str, values: &[&str], ctx: &RequestContext) -> bool {
    if let Some(actual) = resolve_condition_key(cond_key, ctx) {
        values.iter().any(|pattern| string_like_match(pattern, &actual))
    } else {
        false
    }
}

fn string_like_match(pattern: &str, value: &str) -> bool {
    // Simple glob: * matches any sequence, ? matches single char
    string_like_match_recursive(&pattern.chars().collect::<Vec<_>>(), &value.chars().collect::<Vec<_>>(), 0, 0)
}

fn string_like_match_recursive(pattern: &[char], value: &[char], pi: usize, vi: usize) -> bool {
    if pi == pattern.len() && vi == value.len() {
        return true;
    }
    if pi == pattern.len() {
        return false;
    }
    if pattern[pi] == '*' {
        // Try matching * with 0 or more characters
        for i in vi..=value.len() {
            if string_like_match_recursive(pattern, value, pi + 1, i) {
                return true;
            }
        }
        return false;
    }
    if vi == value.len() {
        return false;
    }
    if pattern[pi] == '?' || pattern[pi] == value[vi] {
        return string_like_match_recursive(pattern, value, pi + 1, vi + 1);
    }
    false
}

fn eval_ip_address(cond_key: &str, values: &[&str], ctx: &RequestContext) -> bool {
    if cond_key != "aws:SourceIp" {
        return false;
    }
    let ip = match ctx.source_ip {
        Some(ip) => ip,
        None => return false,
    };
    values.iter().any(|cidr_str| {
        if let Ok(net) = cidr_str.parse::<ipnet::IpNet>() {
            net.contains(&ip)
        } else if let Ok(single_ip) = cidr_str.parse::<IpAddr>() {
            single_ip == ip
        } else {
            false
        }
    })
}

fn eval_date_greater_than(cond_key: &str, values: &[&str], ctx: &RequestContext) -> bool {
    if cond_key != "aws:CurrentTime" {
        return false;
    }
    values.iter().any(|v| {
        if let Ok(dt) = DateTime::parse_from_rfc3339(v) {
            ctx.current_time > dt
        } else {
            false
        }
    })
}

fn eval_date_less_than(cond_key: &str, values: &[&str], ctx: &RequestContext) -> bool {
    if cond_key != "aws:CurrentTime" {
        return false;
    }
    values.iter().any(|v| {
        if let Ok(dt) = DateTime::parse_from_rfc3339(v) {
            ctx.current_time < dt
        } else {
            false
        }
    })
}

fn eval_bool(cond_key: &str, values: &[&str], ctx: &RequestContext) -> bool {
    if let Some(actual) = resolve_condition_key(cond_key, ctx) {
        values.iter().any(|v| *v == actual)
    } else {
        false
    }
}

fn principal_matches(principal: &PolicyPrincipal, principal_id: Option<&str>) -> bool {
    match principal {
        PolicyPrincipal::Wildcard(s) if s == "*" => true,
        PolicyPrincipal::Wildcard(_) => false,
        PolicyPrincipal::Mapped(map) => {
            if let Some(id) = principal_id {
                for values in map.values() {
                    for v in values.as_slice() {
                        if v == "*" || v == id {
                            return true;
                        }
                    }
                }
            }
            false
        }
    }
}

fn action_matches(actions: &OneOrMany<String>, s3_action: &str) -> bool {
    for action in actions.as_slice() {
        if action == "*" || action == "s3:*" {
            return true;
        }
        if action == s3_action {
            return true;
        }
        // Prefix wildcard: "s3:Get*" matches "s3:GetObject"
        if let Some(prefix) = action.strip_suffix('*') {
            if s3_action.starts_with(prefix) {
                return true;
            }
        }
    }
    false
}

fn resource_matches(resources: &OneOrMany<String>, bucket: &str, key: Option<&str>) -> bool {
    let bucket_arn = format!("arn:aws:s3:::{}", bucket);
    let object_arn = key
        .map(|k| format!("arn:aws:s3:::{}/{}", bucket, k))
        .unwrap_or_default();

    for resource in resources.as_slice() {
        if resource == "*" {
            return true;
        }
        // Exact match on bucket ARN
        if resource == &bucket_arn {
            return true;
        }
        // Exact match on object ARN
        if key.is_some() && resource == &object_arn {
            return true;
        }
        // Wildcard suffix: "arn:aws:s3:::bucket/*" matches any object in bucket
        if let Some(prefix) = resource.strip_suffix('*') {
            if bucket_arn.starts_with(prefix) {
                return true;
            }
            if key.is_some() && object_arn.starts_with(prefix) {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::s3::types::{PolicyStatement, PolicyPrincipal, PolicyEffect, OneOrMany};

    fn make_policy(statements: Vec<PolicyStatement>) -> BucketPolicy {
        BucketPolicy {
            version: "2012-10-17".into(),
            statements,
        }
    }

    fn allow_anonymous_get() -> PolicyStatement {
        PolicyStatement {
            sid: Some("AllowAnon".into()),
            effect: PolicyEffect::Allow,
            principal: PolicyPrincipal::Wildcard("*".into()),
            action: OneOrMany::One("s3:GetObject".into()),
            resource: OneOrMany::One("arn:aws:s3:::mybucket/*".into()),
            condition: None,
        }
    }

    #[test]
    fn test_allow_anonymous_get() {
        let policy = make_policy(vec![allow_anonymous_get()]);
        let decision = evaluate_policy(&policy, "s3:GetObject", "mybucket", Some("file.txt"), None, None);
        assert_eq!(decision, PolicyDecision::ExplicitAllow);
    }

    #[test]
    fn test_deny_trumps_allow() {
        let policy = make_policy(vec![
            allow_anonymous_get(),
            PolicyStatement {
                sid: Some("DenyAll".into()),
                effect: PolicyEffect::Deny,
                principal: PolicyPrincipal::Wildcard("*".into()),
                action: OneOrMany::One("s3:GetObject".into()),
                resource: OneOrMany::One("arn:aws:s3:::mybucket/*".into()),
                condition: None,
            },
        ]);
        let decision = evaluate_policy(&policy, "s3:GetObject", "mybucket", Some("file.txt"), None, None);
        assert_eq!(decision, PolicyDecision::ExplicitDeny);
    }

    #[test]
    fn test_implicit_deny() {
        let policy = make_policy(vec![allow_anonymous_get()]);
        let decision = evaluate_policy(&policy, "s3:PutObject", "mybucket", Some("file.txt"), None, None);
        assert_eq!(decision, PolicyDecision::ImplicitDeny);
    }

    #[test]
    fn test_action_wildcard() {
        let policy = make_policy(vec![PolicyStatement {
            sid: None,
            effect: PolicyEffect::Allow,
            principal: PolicyPrincipal::Wildcard("*".into()),
            action: OneOrMany::One("s3:Get*".into()),
            resource: OneOrMany::One("arn:aws:s3:::mybucket/*".into()),
            condition: None,
        }]);
        let decision = evaluate_policy(&policy, "s3:GetObject", "mybucket", Some("f"), None, None);
        assert_eq!(decision, PolicyDecision::ExplicitAllow);
        let decision = evaluate_policy(&policy, "s3:GetObjectTagging", "mybucket", Some("f"), None, None);
        assert_eq!(decision, PolicyDecision::ExplicitAllow);
        let decision = evaluate_policy(&policy, "s3:PutObject", "mybucket", Some("f"), None, None);
        assert_eq!(decision, PolicyDecision::ImplicitDeny);
    }

    #[test]
    fn test_principal_specific_key_id() {
        use std::collections::HashMap;
        let mut map = HashMap::new();
        map.insert("AWS".into(), OneOrMany::One("AKID123".into()));
        let policy = make_policy(vec![PolicyStatement {
            sid: None,
            effect: PolicyEffect::Allow,
            principal: PolicyPrincipal::Mapped(map),
            action: OneOrMany::One("s3:GetObject".into()),
            resource: OneOrMany::One("arn:aws:s3:::mybucket/*".into()),
            condition: None,
        }]);
        let decision = evaluate_policy(&policy, "s3:GetObject", "mybucket", Some("f"), Some("AKID123"), None);
        assert_eq!(decision, PolicyDecision::ExplicitAllow);
        let decision = evaluate_policy(&policy, "s3:GetObject", "mybucket", Some("f"), Some("OTHER"), None);
        assert_eq!(decision, PolicyDecision::ImplicitDeny);
        let decision = evaluate_policy(&policy, "s3:GetObject", "mybucket", Some("f"), None, None);
        assert_eq!(decision, PolicyDecision::ImplicitDeny);
    }

    #[test]
    fn test_condition_string_equals() {
        let mut condition = std::collections::HashMap::new();
        let mut inner = std::collections::HashMap::new();
        inner.insert(
            "s3:prefix".into(),
            OneOrMany::One("logs/".into()),
        );
        condition.insert("StringEquals".into(), inner);

        let policy = make_policy(vec![PolicyStatement {
            sid: None,
            effect: PolicyEffect::Allow,
            principal: PolicyPrincipal::Wildcard("*".into()),
            action: OneOrMany::One("s3:ListBucket".into()),
            resource: OneOrMany::One("arn:aws:s3:::mybucket".into()),
            condition: Some(condition),
        }]);

        let ctx = RequestContext {
            source_ip: None,
            current_time: Utc::now(),
            secure_transport: false,
            s3_prefix: Some("logs/".into()),
        };
        let decision = evaluate_policy(&policy, "s3:ListBucket", "mybucket", None, None, Some(&ctx));
        assert_eq!(decision, PolicyDecision::ExplicitAllow);

        // Non-matching prefix
        let ctx2 = RequestContext {
            s3_prefix: Some("other/".into()),
            ..ctx
        };
        let decision = evaluate_policy(&policy, "s3:ListBucket", "mybucket", None, None, Some(&ctx2));
        assert_eq!(decision, PolicyDecision::ImplicitDeny);
    }

    #[test]
    fn test_condition_ip_address() {
        let mut condition = std::collections::HashMap::new();
        let mut inner = std::collections::HashMap::new();
        inner.insert(
            "aws:SourceIp".into(),
            OneOrMany::One("10.0.0.0/8".into()),
        );
        condition.insert("IpAddress".into(), inner);

        let policy = make_policy(vec![PolicyStatement {
            sid: None,
            effect: PolicyEffect::Allow,
            principal: PolicyPrincipal::Wildcard("*".into()),
            action: OneOrMany::One("s3:GetObject".into()),
            resource: OneOrMany::One("arn:aws:s3:::mybucket/*".into()),
            condition: Some(condition),
        }]);

        let ctx = RequestContext {
            source_ip: Some("10.1.2.3".parse().unwrap()),
            current_time: Utc::now(),
            secure_transport: false,
            s3_prefix: None,
        };
        let decision = evaluate_policy(&policy, "s3:GetObject", "mybucket", Some("f"), None, Some(&ctx));
        assert_eq!(decision, PolicyDecision::ExplicitAllow);

        // IP outside CIDR
        let ctx2 = RequestContext {
            source_ip: Some("192.168.1.1".parse().unwrap()),
            ..ctx
        };
        let decision = evaluate_policy(&policy, "s3:GetObject", "mybucket", Some("f"), None, Some(&ctx2));
        assert_eq!(decision, PolicyDecision::ImplicitDeny);
    }

    #[test]
    fn test_condition_date() {
        let mut condition = std::collections::HashMap::new();
        let mut inner = std::collections::HashMap::new();
        inner.insert(
            "aws:CurrentTime".into(),
            OneOrMany::One("2030-01-01T00:00:00+00:00".into()),
        );
        condition.insert("DateLessThan".into(), inner);

        let policy = make_policy(vec![PolicyStatement {
            sid: None,
            effect: PolicyEffect::Allow,
            principal: PolicyPrincipal::Wildcard("*".into()),
            action: OneOrMany::One("s3:GetObject".into()),
            resource: OneOrMany::One("arn:aws:s3:::mybucket/*".into()),
            condition: Some(condition),
        }]);

        let ctx = RequestContext {
            source_ip: None,
            current_time: Utc::now(), // Should be before 2030
            secure_transport: false,
            s3_prefix: None,
        };
        let decision = evaluate_policy(&policy, "s3:GetObject", "mybucket", Some("f"), None, Some(&ctx));
        assert_eq!(decision, PolicyDecision::ExplicitAllow);
    }

    #[test]
    fn test_condition_no_context_skips() {
        let mut condition = std::collections::HashMap::new();
        let mut inner = std::collections::HashMap::new();
        inner.insert(
            "aws:SourceIp".into(),
            OneOrMany::One("10.0.0.0/8".into()),
        );
        condition.insert("IpAddress".into(), inner);

        let policy = make_policy(vec![PolicyStatement {
            sid: None,
            effect: PolicyEffect::Allow,
            principal: PolicyPrincipal::Wildcard("*".into()),
            action: OneOrMany::One("s3:GetObject".into()),
            resource: OneOrMany::One("arn:aws:s3:::mybucket/*".into()),
            condition: Some(condition),
        }]);

        // With no context, statement with conditions should be skipped
        let decision = evaluate_policy(&policy, "s3:GetObject", "mybucket", Some("f"), None, None);
        assert_eq!(decision, PolicyDecision::ImplicitDeny);
    }

    #[test]
    fn test_condition_bool_secure_transport() {
        let mut condition = std::collections::HashMap::new();
        let mut inner = std::collections::HashMap::new();
        inner.insert(
            "aws:SecureTransport".into(),
            OneOrMany::One("true".into()),
        );
        condition.insert("Bool".into(), inner);

        let policy = make_policy(vec![PolicyStatement {
            sid: None,
            effect: PolicyEffect::Deny,
            principal: PolicyPrincipal::Wildcard("*".into()),
            action: OneOrMany::One("s3:*".into()),
            resource: OneOrMany::One("*".into()),
            condition: Some(condition),
        }]);

        // Secure transport = true → condition matches → deny applies
        let ctx = RequestContext {
            source_ip: None,
            current_time: Utc::now(),
            secure_transport: true,
            s3_prefix: None,
        };
        let decision = evaluate_policy(&policy, "s3:GetObject", "mybucket", Some("f"), None, Some(&ctx));
        assert_eq!(decision, PolicyDecision::ExplicitDeny);

        // Secure transport = false → condition doesn't match → deny doesn't apply
        let ctx2 = RequestContext {
            secure_transport: false,
            ..ctx
        };
        let decision = evaluate_policy(&policy, "s3:GetObject", "mybucket", Some("f"), None, Some(&ctx2));
        assert_eq!(decision, PolicyDecision::ImplicitDeny);
    }
}
