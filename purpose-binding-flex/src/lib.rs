// Copyright 2026 Salesforce, Inc. All rights reserved.
//! Purpose Binding — MCP-native, inbound, request-gating Omni/Flex Gateway policy.
//!
//! Every MCP `tools/call` (and any other configured method) must declare a
//! processing purpose — from a request header or a JWT claim — that is permitted
//! for the target tool. Missing or disallowed purposes are rejected (fail-closed).
//! GDPR Art. 5(1)(b) purpose limitation at the agent boundary.

mod claim;
mod generated;
mod purpose;

use std::sync::Arc;

use anyhow::{anyhow, Result};
use pdk::hl::*;
use pdk::logger;
use serde_json::{json, Value};

use crate::generated::config::Config;
use crate::purpose::Outcome;

const TOOLS_CALL: &str = "tools/call";

/// Build the JSON-RPC deny response. HTTP 403 (deliberate) so a caller can route
/// on status; the JSON-RPC error carries the reason in-band.
fn deny_response(id: Value, message: &str) -> Response {
    let body = json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": -32002, "message": message }
    })
    .to_string();
    Response::new(403)
        .with_headers(vec![("content-type".to_string(), "application/json".to_string())])
        .with_body(body.into_bytes())
}

/// The entity key a method is keyed on (tool name for tools/call & prompts/get,
/// uri for resources/read).
fn entity_key(params: &Value) -> Option<String> {
    params
        .get("name")
        .and_then(Value::as_str)
        .or_else(|| params.get("uri").and_then(Value::as_str))
        .map(str::to_string)
}

async fn request_filter(request_state: RequestState, config: Arc<Config>) -> Flow<()> {
    let headers_state = request_state.into_headers_state().await;

    if headers_state.method().as_str() != "POST" {
        return Flow::Continue(());
    }
    match headers_state.handler().header("content-type") {
        Some(ct) if ct.starts_with("application/json") => {}
        _ => return Flow::Continue(()),
    }

    // --- extract the declared purpose (from the header phase) ---
    let source = config.source.as_deref().unwrap_or("header");
    let declared: Option<String> = if source == "claim" {
        let claim_name = config.claim_name.as_deref().unwrap_or("purpose");
        headers_state
            .handler()
            .header("authorization")
            .and_then(|a| claim::claim_from_authorization(&a, claim_name))
    } else {
        let header_name = config.header_name.as_deref().unwrap_or("x-processing-purpose");
        headers_state.handler().header(header_name)
    };

    // --- read the JSON-RPC envelope from the body ---
    let body_state = headers_state.into_body_state().await;
    let body = body_state.handler().body();
    let req: Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(_) => return Flow::Continue(()), // not JSON-RPC → not a governed call
    };

    let method = req.get("method").and_then(Value::as_str).unwrap_or_default();

    // Which methods do we enforce on? Default: tools/call only.
    let enforced = match &config.enforce_methods {
        Some(m) if !m.is_empty() => m.iter().any(|x| x == method),
        _ => method == TOOLS_CALL,
    };
    if !enforced {
        return Flow::Continue(());
    }

    let params = req.get("params").cloned().unwrap_or(Value::Null);
    let tool = match entity_key(&params) {
        Some(t) => t,
        None => return Flow::Continue(()), // nothing to key on
    };

    // --- resolve allow-list & evaluate ---
    let per_tool: Vec<(String, Vec<String>)> = config
        .tool_purposes
        .iter()
        .flatten()
        .map(|t| (t.tool.clone(), t.purposes.clone()))
        .collect();
    let global = config.allowed_purposes.clone().unwrap_or_default();
    let allowed = purpose::resolve_allowed(&tool, &per_tool, &global);

    let fail_closed = config.fail_mode.as_deref().unwrap_or("closed") != "open";
    let id = req.get("id").cloned().unwrap_or(Value::Null);

    match purpose::evaluate(allowed, declared.as_deref(), fail_closed) {
        Outcome::Allow | Outcome::NotGoverned => Flow::Continue(()),
        Outcome::DenyMissing => {
            logger::info!("purpose-binding: denied tool '{tool}' — no purpose declared");
            Flow::Break(deny_response(
                id,
                &format!("purpose required for tool '{tool}' but none was declared"),
            ))
        }
        Outcome::DenyNotPermitted => {
            let p = declared.unwrap_or_default();
            logger::info!("purpose-binding: denied tool '{tool}' — purpose '{p}' not permitted");
            Flow::Break(deny_response(
                id,
                &format!("purpose '{p}' is not permitted for tool '{tool}'"),
            ))
        }
    }
}

#[entrypoint]
async fn configure(launcher: Launcher, Configuration(bytes): Configuration) -> Result<()> {
    let config: Config = serde_json::from_slice(&bytes).map_err(|err| {
        anyhow!(
            "Failed to parse configuration '{}'. Cause: {}",
            String::from_utf8_lossy(&bytes),
            err
        )
    })?;
    let config = Arc::new(config);
    launcher
        .launch(on_request(move |rs| request_filter(rs, config.clone())))
        .await?;
    Ok(())
}
