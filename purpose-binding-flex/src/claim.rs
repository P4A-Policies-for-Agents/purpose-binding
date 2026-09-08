// Copyright 2026 Salesforce, Inc. All rights reserved.
//! Reads a claim from a JWT's payload. The signature is NOT verified here — pair
//! this policy with a JWT Validation policy in front. Pure — unit-testable.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use serde_json::Value;

/// Extract a string claim from a `Bearer <jwt>` Authorization header value.
pub fn claim_from_authorization(authorization: &str, claim: &str) -> Option<String> {
    let token = authorization
        .strip_prefix("Bearer ")
        .or_else(|| authorization.strip_prefix("bearer "))
        .unwrap_or(authorization)
        .trim();
    let payload_b64 = token.split('.').nth(1)?; // header.payload.signature
    let bytes = URL_SAFE_NO_PAD.decode(payload_b64.trim_end_matches('=')).ok()?;
    let json: Value = serde_json::from_slice(&bytes).ok()?;
    json.get(claim).and_then(Value::as_str).map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_jwt(payload: &str) -> String {
        let b = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(payload.as_bytes());
        format!("hdr.{b}.sig")
    }

    #[test]
    fn reads_claim() {
        let jwt = make_jwt(r#"{"sub":"u1","purpose":"support"}"#);
        assert_eq!(
            claim_from_authorization(&format!("Bearer {jwt}"), "purpose"),
            Some("support".to_string())
        );
    }

    #[test]
    fn missing_claim_is_none() {
        let jwt = make_jwt(r#"{"sub":"u1"}"#);
        assert_eq!(claim_from_authorization(&format!("Bearer {jwt}"), "purpose"), None);
    }

    #[test]
    fn garbage_is_none() {
        assert_eq!(claim_from_authorization("Bearer not-a-jwt", "purpose"), None);
    }
}
