// Copyright 2026 Salesforce, Inc. All rights reserved.
//
// Integration test (requires Docker; run with `make test`). Composes a local Flex
// instance + an HttpMock backend and asserts the Purpose Binding policy allows a
// permitted purpose and rejects a disallowed one. The pure decision logic is
// covered by `cargo test --lib`.

mod common;

use httpmock::MockServer;
use pdk_test::port::Port;
use pdk_test::services::flex::{ApiConfig, Flex, FlexConfig, PolicyConfig};
use pdk_test::services::httpmock::{HttpMock, HttpMockConfig};
use pdk_test::{pdk_test, TestComposite};

use common::*;

const FLEX_PORT: Port = 8081;

#[pdk_test]
async fn allows_permitted_denies_disallowed_purpose() -> anyhow::Result<()> {
    let httpmock_config = HttpMockConfig::builder()
        .port(80)
        .version("latest")
        .hostname("backend")
        .build();

    let policy_config = PolicyConfig::builder()
        .name(POLICY_NAME)
        .configuration(serde_json::json!({
            "source": "header",
            "headerName": "x-processing-purpose",
            "toolPurposes": [
                { "tool": "lookup_customer", "purposes": ["customer_support", "billing"] }
            ],
            "failMode": "closed"
        }))
        .build();

    let api_config = ApiConfig::builder()
        .name("myApi")
        .upstream(&httpmock_config)
        .path("/")
        .port(FLEX_PORT)
        .policies([policy_config])
        .build();

    let flex_config = FlexConfig::builder()
        .version("1.10.0")
        .hostname("local-flex")
        .with_api(api_config)
        .config_mounts([(POLICY_DIR, "policy"), (COMMON_CONFIG_DIR, "common")])
        .build();

    let composite = TestComposite::builder()
        .with_service(flex_config)
        .with_service(httpmock_config)
        .build()
        .await?;

    let flex: Flex = composite.service()?;
    let flex_url = flex.external_url(FLEX_PORT).unwrap();
    let httpmock: HttpMock = composite.service()?;
    let mock_server = MockServer::connect_async(httpmock.socket()).await;

    // Upstream answers any POST with a JSON-RPC tools/call result.
    mock_server
        .mock_async(|when, then| {
            when.method(httpmock::Method::POST);
            then.status(200)
                .header("content-type", "application/json")
                .body(r#"{"jsonrpc":"2.0","id":2,"result":{"structuredContent":{"fullName":"Dana Whitfield"}}}"#);
        })
        .await;

    let client = reqwest::Client::new();
    let call = r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"lookup_customer","arguments":{"customerId":"C-1"}}}"#;

    // permitted purpose -> allowed (reaches upstream, 200)
    let ok = client
        .post(format!("{flex_url}/"))
        .header("content-type", "application/json")
        .header("x-processing-purpose", "customer_support")
        .body(call)
        .send()
        .await?;
    assert_eq!(ok.status(), 200);

    // disallowed purpose -> denied (403, blocked at gateway)
    let denied = client
        .post(format!("{flex_url}/"))
        .header("content-type", "application/json")
        .header("x-processing-purpose", "marketing")
        .body(call)
        .send()
        .await?;
    assert_eq!(denied.status(), 403);

    // missing purpose -> denied (403)
    let missing = client
        .post(format!("{flex_url}/"))
        .header("content-type", "application/json")
        .body(call)
        .send()
        .await?;
    assert_eq!(missing.status(), 403);

    Ok(())
}
