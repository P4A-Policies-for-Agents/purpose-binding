// Copyright 2026 Salesforce, Inc. All rights reserved.
//! Pure purpose-binding decision logic. No PDK imports — fully unit-testable.

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Purpose permitted (or open-mode pass) — let the call through.
    Allow,
    /// The tool has no configured allow-list — policy does not govern it.
    NotGoverned,
    /// Governed tool, but no purpose was declared.
    DenyMissing,
    /// Governed tool, but the declared purpose is not in the allow-list.
    DenyNotPermitted,
}

/// Resolve the allow-list for a tool: a per-tool entry wins; otherwise the global
/// list. Returns `None` when neither is configured (⇒ tool is not governed).
pub fn resolve_allowed<'a>(
    tool: &str,
    per_tool: &'a [(String, Vec<String>)],
    global: &'a [String],
) -> Option<&'a [String]> {
    for (t, purposes) in per_tool {
        if t == tool {
            return Some(purposes.as_slice());
        }
    }
    if global.is_empty() {
        None
    } else {
        Some(global)
    }
}

/// Decide the outcome for a governed call.
pub fn evaluate(allowed: Option<&[String]>, declared: Option<&str>, fail_closed: bool) -> Outcome {
    let allowed = match allowed {
        Some(a) => a,
        None => return Outcome::NotGoverned,
    };
    match declared {
        None => {
            if fail_closed {
                Outcome::DenyMissing
            } else {
                Outcome::Allow
            }
        }
        Some(p) => {
            if allowed.iter().any(|a| a == p) {
                Outcome::Allow
            } else if fail_closed {
                Outcome::DenyNotPermitted
            } else {
                Outcome::Allow
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn resolve_per_tool_wins() {
        let per = vec![("lookup_customer".to_string(), s(&["support", "billing"]))];
        let global = s(&["analytics"]);
        assert_eq!(
            resolve_allowed("lookup_customer", &per, &global),
            Some(s(&["support", "billing"]).as_slice())
        );
    }

    #[test]
    fn resolve_falls_back_to_global() {
        let per: Vec<(String, Vec<String>)> = vec![];
        let global = s(&["analytics"]);
        assert_eq!(resolve_allowed("other", &per, &global), Some(s(&["analytics"]).as_slice()));
    }

    #[test]
    fn resolve_not_governed_when_empty() {
        let per: Vec<(String, Vec<String>)> = vec![];
        let global: Vec<String> = vec![];
        assert_eq!(resolve_allowed("other", &per, &global), None);
    }

    #[test]
    fn evaluate_allow_when_permitted() {
        let allowed = s(&["support", "billing"]);
        assert_eq!(evaluate(Some(&allowed), Some("support"), true), Outcome::Allow);
    }

    #[test]
    fn evaluate_deny_not_permitted_closed() {
        let allowed = s(&["support"]);
        assert_eq!(evaluate(Some(&allowed), Some("marketing"), true), Outcome::DenyNotPermitted);
    }

    #[test]
    fn evaluate_deny_missing_closed() {
        let allowed = s(&["support"]);
        assert_eq!(evaluate(Some(&allowed), None, true), Outcome::DenyMissing);
    }

    #[test]
    fn evaluate_open_mode_allows_disallowed_and_missing() {
        let allowed = s(&["support"]);
        assert_eq!(evaluate(Some(&allowed), Some("marketing"), false), Outcome::Allow);
        assert_eq!(evaluate(Some(&allowed), None, false), Outcome::Allow);
    }

    #[test]
    fn evaluate_not_governed_passes() {
        assert_eq!(evaluate(None, None, true), Outcome::NotGoverned);
    }
}
