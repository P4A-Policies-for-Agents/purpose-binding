# Purpose Binding

An **MCP-native, inbound, request-gating** MuleSoft Omni/Flex Gateway custom
policy. Every MCP `tools/call` (and any other configured method) must declare a
**processing purpose** — from a request header or a JWT claim — that is
**permitted for the target tool**. Missing or disallowed purposes are rejected at
the gateway, before the tool is ever invoked. This is the GDPR Art. 5(1)(b)
purpose-limitation control at the agent boundary: the gateway can already answer
"*may this caller reach this tool*", but not "*is there a lawful basis for this
processing*" — Purpose Binding adds that.

Built with the PDK, Rust → `wasm32-wasip1`, split-model.

> **Key caveat — disclosure of a purpose is not proof of it.** The policy checks
> that a *declared* purpose is in the tool's allow-list; it forces an explicit,
> auditable purpose on every governed call and makes purpose drift a hard failure.
> With `source: claim` the JWT signature is **not** verified here — pair with a
> JWT Validation policy in front.

## How it decides

For a governed method (default `tools/call`):
1. Resolve the tool's allow-list — a `toolPurposes` entry for the tool wins, else
   the global `allowedPurposes`. If neither is configured, the tool is **not
   governed** and passes.
2. Read the declared purpose from `headerName` (source `header`) or the JWT
   `claimName` (source `claim`).
3. `Allow` if the purpose is in the allow-list; otherwise **deny** (`failMode:
   closed`) with **HTTP 403** + a JSON-RPC error, or pass + log (`failMode: open`).

Non-governed methods (`initialize`, `tools/list`, notifications, …) always pass,
so the MCP handshake is unaffected.

## Configuration

| Property | Type | Default | Description |
|---|---|---|---|
| `source` | `header`\|`claim` | `header` | Where the declared purpose comes from. |
| `headerName` | string | `x-processing-purpose` | Purpose header (source `header`). |
| `claimName` | string | `purpose` | JWT claim (source `claim`; read from the `Authorization` Bearer token, signature not verified). |
| `allowedPurposes` | array<string> | `[]` | Global fallback allow-list (used when a tool has no `toolPurposes` entry). |
| `toolPurposes` | array | `[]` | Per-tool allow-lists: `{ tool, purposes[] }`. |
| `enforceMethods` | array<string> | `[]` | Methods to enforce on; empty = `tools/call` only. |
| `failMode` | `closed`\|`open` | `closed` | Behavior for a governed call with a missing/disallowed purpose. |

### Example config

```json
{
  "source": "header",
  "headerName": "x-processing-purpose",
  "toolPurposes": [
    { "tool": "lookup_customer", "purposes": ["customer_support", "billing"] }
  ],
  "failMode": "closed"
}
```

## Repository layout (split-model)

```
purpose-binding-definition/   # gcl.yaml (schema), exchange.json, Makefile
purpose-binding-flex/          # Rust implementation
  src/lib.rs        # entrypoint + inbound request filter
  src/purpose.rs    # pure decision logic (resolve allow-list + evaluate) — unit tested
  src/claim.rs      # JWT-claim reader for source=claim — unit tested
  src/generated/    # config.rs generated from gcl.yaml
demo/               # A2D mock manifest, policy config, live demo script
```

## Build, test, release

```bash
cd purpose-binding-definition && make release     # publish definition
cd ../purpose-binding-flex
make build-asset-files
cargo build --target wasm32-wasip1 --release
cargo test --lib                                  # 11 pure unit tests (no Docker)
make release                                      # publish implementation
```

## Live demo — purpose limitation on `lookup_customer`

An A2D mock MCP server (`lookup_customer`) fronted by a managed Flex Gateway with
this policy applied (`toolPurposes: lookup_customer → [customer_support, billing]`).

```bash
cp demo/env.local.sh.example demo/env.local.sh   # fill in your governed endpoint
./demo/demo.sh
```

Shows: `initialize` passes; `purpose=customer_support` → **ALLOW** (profile
returned); `purpose=marketing` → **DENY 403**; no purpose → **DENY 403**.

Full provisioning in [`demo/PROVISION.md`](demo/PROVISION.md).

## Skills used

- **PDK** (`omni-gateway-pdk-skills`): `pdk-create-policy`, `pdk-mcp`,
  `pdk-schema-definition`, `pdk-request-headers-bodies`, `pdk-stop-execution`.
- **P4A** (`p4a-skills`): `p4a-build-policy`, `p4a-verify-requirements`,
  `p4a-mcp-usage`, `p4a-test-mcp-policies-with-a2d`.
