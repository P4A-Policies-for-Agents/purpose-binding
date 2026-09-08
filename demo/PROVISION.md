# Demo provisioning runbook

Steps to stand up the live Purpose Binding demo, following
`p4a-test-mcp-policies-with-a2d`. Done with `anypoint-cli-v4` (already
authenticated) + the A2D MCP tools — **no bearer token / connected-app secret
needed**. Replace the `<...>` placeholders with your own values (identifiers, not
secrets — but keep them out of a public repo).

| Placeholder | What it is | How to get it |
|---|---|---|
| `<orgId>` | Business-group / org id | `anypoint-cli-v4 account:business-group:list` |
| `<mockServerId>` | A2D mock MCP server id | returned by `design_mcp_server` |
| `<gatewayId>` | Managed Flex Gateway **resource** id | `runtime-mgr:gateways:managed:list --environment Sandbox` |
| `<gatewayPublicHost>` | Gateway public ingress host | `runtime-mgr:gateways:managed:describe <gatewayId>` → `configuration.ingress.publicUrl` |
| `<apiInstanceId>` | API Manager instance id | returned by `api-mgr:api:manage` |

Mock surface: `https://www.a2d-ai.com/api/platform/<mockServerId>/mcp`
Governed endpoint: `https://<gatewayPublicHost>/purpose-binding-demo/mcp`

## 1. A2D mock (A2D MCP tools)

`design_mcp_server` (type `mock`, provider org + URL) → `add_mcp_tool`
`lookup_customer` (schemas meeting A2D's quality gate: tool desc ≥200 chars,
property desc ≥25, property name ≥5) with a mock scenario returning a profile.

## 2. Publish the manifest to Exchange as `type=mcp`

```bash
anypoint-cli-v4 exchange:asset:upload \
  --name "Purpose Binding Test Server" --type mcp --status published \
  --description "Mock Customer 360 MCP server for the Purpose Binding policy demo" \
  --properties='{"platform":"a2d"}' \
  --files='{"mcp-metadata.json":"./mcp-metadata.json"}' \
  purpose-binding-test-server/1.0.0
```

## 3. Create + deploy the MCP Flex API instance

```bash
anypoint-cli-v4 api-mgr:api:manage purpose-binding-test-server 1.0.0 <orgId> \
  --environment Sandbox --isFlex --type mcp \
  --uri "https://www.a2d-ai.com/api/platform/<mockServerId>/" \
  --apiInstanceLabel "purpose-binding-demo"

anypoint-cli-v4 api-mgr:api:edit <apiInstanceId> --environment Sandbox --isFlex --type mcp \
  --withProxy --scheme http --port 8081 --path "/purpose-binding-demo/" \
  --uri "https://www.a2d-ai.com/api/platform/<mockServerId>/"

# target = the GATEWAY resource id (not its targetId)
anypoint-cli-v4 api-mgr:api:deploy <apiInstanceId> --environment Sandbox \
  --target <gatewayId> --gatewayVersion 1.0.0 --overwrite
```

## 4. Apply the policy

```bash
anypoint-cli-v4 api-mgr:policy:apply <apiInstanceId> purpose-binding \
  --environment Sandbox --groupId <orgId> \
  --policyVersion 1.0.0 --configFile ./config.json
anypoint-cli-v4 api-mgr:api:redeploy <apiInstanceId> --environment Sandbox
```

## 5. Verify

```bash
cp env.local.sh.example env.local.sh   # set PB_GW_URL to your governed endpoint
./demo.sh
```

Expected: `initialize` OK; `purpose=customer_support` → 200 + profile;
`purpose=marketing` → 403; no purpose → 403.

## Notes

- **Purpose Binding is inbound and request-gating** — it rejects on the request
  leg before the upstream MCP server is called; no response rewrite, so **MCP
  Support is not required** for it (the mcp endpoint proxies MCP natively).
- Deny returns **HTTP 403** with a JSON-RPC `-32002` error body (deliberate — so a
  caller can route on status). Non-governed methods (`initialize`, `tools/list`)
  always pass.
- Upstream URI is the mock surface **minus** `/mcp`, trailing slash. Deploy target
  is the gateway **resource** id; `api:manage` alone leaves `deployment: null`.
