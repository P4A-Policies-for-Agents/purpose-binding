# Purpose Binding — MuleSoft Omni/Flex Gateway Policy

An **MCP-native, inbound, request-gating** custom policy for the MuleSoft
Omni/Flex Gateway. It enforces **purpose limitation**: every governed MCP
`tools/call` must declare a **processing purpose** — from a request header or a
JWT claim — that is **permitted for the target tool**. Calls with a missing or
disallowed purpose are rejected **at the gateway, before the tool is ever
invoked**.

Built with the Policy Development Kit (PDK), Rust → `wasm32-wasip1`, split-model
(definition + implementation), no external service dependency.

---

## Table of contents

- [Why this policy](#why-this-policy)
- [What it does](#what-it-does)
- [How it decides](#how-it-decides)
- [Request lifecycle](#request-lifecycle)
- [Configuration reference](#configuration-reference)
- [Worked examples](#worked-examples)
- [Repository layout](#repository-layout)
- [Build, test & release](#build-test--release)
- [Live demo](#live-demo)
- [Composition & positioning](#composition--positioning)
- [Design notes & gotchas](#design-notes--gotchas)
- [Security considerations](#security-considerations)
- [Skills used](#skills-used)

---

## Why this policy

A gateway can already answer *"may this caller reach this tool?"* (authentication,
access control). It cannot, out of the box, answer *"is there a lawful basis for
this processing?"* — the **purpose limitation** principle of **GDPR Art. 5(1)(b)**
and a recurring requirement in the EU AI Act traceability obligations.

Agents make this sharper: the same tool (`lookup_customer`, `export_dataset`,
`get_patient_record`) may be legitimate for one purpose (customer support) and
unlawful for another (marketing). Purpose Binding turns "purpose" into a
first-class, enforced, auditable input on every governed call — and makes
**purpose drift** a hard failure rather than a silent one.

> **Key caveat — disclosure of a purpose is not proof of it.** The policy verifies
> that a *declared* purpose is in the tool's allow-list. Its value is (a) forcing
> an explicit, auditable purpose on every governed call, and (b) making an
> unlisted purpose a hard rejection. With `source: claim` the JWT signature is
> **not** verified here — pair with a JWT Validation policy in front so the claim
> is trustworthy.

---

## What it does

- Inspects inbound MCP JSON-RPC traffic (streamable-HTTP / `application/json`).
- Enforces on `tools/call` by default (configurable via `enforceMethods`; also
  supports `resources/read`, `prompts/get` by keying on `params.uri` / `params.name`).
- **Passes non-governed methods through untouched** — `initialize`, `tools/list`,
  `notifications/*` — so the MCP handshake is never broken.
- On a denial, returns **HTTP 403** with an in-band **JSON-RPC error** (`-32002`)
  describing the reason.

It never contacts an external service and holds no state — the decision is a pure
function of the request and the policy config.

---

## How it decides

For each governed call:

1. **Resolve the allow-list for the tool**
   - a matching `toolPurposes` entry wins;
   - otherwise the global `allowedPurposes`;
   - if neither is configured → the tool is **not governed** → **allow**.
2. **Read the declared purpose** from `headerName` (source `header`) or the JWT
   `claimName` (source `claim`).
3. **Evaluate**:

| Allow-list | Declared purpose | `failMode: closed` | `failMode: open` |
|---|---|---|---|
| none configured | — | **Allow** (not governed) | Allow |
| configured | in allow-list | **Allow** | Allow |
| configured | not in allow-list | **Deny** `403` | Allow (log) |
| configured | missing | **Deny** `403` | Allow (log) |

`closed` is the default and the correct posture for a compliance control.

---

## Request lifecycle

```
client/agent ──POST JSON-RPC──►  Flex Gateway (Purpose Binding, inbound)
                                     │
                    read purpose (header or JWT claim)  ── headers phase
                    parse JSON-RPC envelope             ── body phase
                                     │
             method governed?  ──no──►  Flow::Continue ─────────────┐
                    │yes                                            │
             resolve allow-list for tool                           │
             evaluate(purpose)                                     ▼
              ├─ Allow / NotGoverned ─► Flow::Continue ─► upstream MCP server
              └─ Deny ─► Flow::Break(HTTP 403 + JSON-RPC -32002)   (upstream never called)
```

The policy reads the purpose header in the **headers phase**, then transitions to
the **body phase** to parse the JSON-RPC envelope — headers must be read before
the body per the PDK event-flow model.

---

## Configuration reference

| Property | Type | Default | Description |
|---|---|---|---|
| `source` | `header` \| `claim` | `header` | Where the declared purpose comes from. |
| `headerName` | string | `x-processing-purpose` | Purpose header name (source `header`). |
| `claimName` | string | `purpose` | JWT claim name (source `claim`), read from the `Authorization: Bearer` token. Signature **not** verified here. |
| `allowedPurposes` | array\<string\> | `[]` | Global fallback allow-list, used for a tool with no `toolPurposes` entry. Empty ⇒ such tools are not governed. |
| `toolPurposes` | array\<object\> | `[]` | Per-tool allow-lists. Each: `{ "tool": "<name>", "purposes": ["…"] }`. Overrides `allowedPurposes` for that tool. |
| `enforceMethods` | array\<string\> | `[]` | MCP methods to enforce on. Empty ⇒ `tools/call` only. |
| `failMode` | `closed` \| `open` | `closed` | Behavior for a governed call with a missing/disallowed purpose. |

**Denial response** (HTTP 403, MCP JSON-RPC convention body):

```json
{ "jsonrpc": "2.0", "id": 2, "error": { "code": -32002, "message": "purpose 'marketing' is not permitted for tool 'lookup_customer'" } }
```

---

## Worked examples

### Header-based (per-tool allow-lists)

```json
{
  "source": "header",
  "headerName": "x-processing-purpose",
  "toolPurposes": [
    { "tool": "lookup_customer",  "purposes": ["customer_support", "billing"] },
    { "tool": "export_dataset",   "purposes": ["analytics_aggregate"] }
  ],
  "failMode": "closed"
}
```

```bash
# allowed
curl -H 'x-processing-purpose: customer_support' … tools/call lookup_customer   # 200
# denied
curl -H 'x-processing-purpose: marketing'         … tools/call lookup_customer   # 403
```

### JWT-claim based (global allow-list, enforce more methods)

```json
{
  "source": "claim",
  "claimName": "purpose",
  "allowedPurposes": ["treatment", "care_coordination"],
  "enforceMethods": ["tools/call", "resources/read"],
  "failMode": "closed"
}
```

The purpose is read from the `purpose` claim of the `Authorization: Bearer <jwt>`
token. Put a **JWT Validation** policy before this one so the token is verified.

---

## Repository layout

```
purpose-binding-definition/     # policy DEFINITION (Exchange asset)
  gcl.yaml                      # config schema (properties, enums, defaults)
  exchange.json                 # Exchange coordinates (groupId/assetId/version)
  Makefile                      # build / publish / release

purpose-binding-flex/           # policy IMPLEMENTATION (Rust → WASM)
  src/lib.rs                    # entrypoint + inbound request filter
  src/purpose.rs                # PURE decision logic (resolve + evaluate) — unit tested
  src/claim.rs                  # JWT-claim reader for source=claim — unit tested
  src/generated/config.rs       # generated from gcl.yaml by `make build-asset-files`
  tests/requests.rs             # Docker-based integration test (make test)
  Cargo.toml / Makefile

demo/
  mcp-metadata.json             # MCP manifest published to Exchange as type=mcp
  config.json                   # policy config applied in the demo
  demo.sh                       # live allow/deny demo (reads endpoint from env.local.sh)
  env.local.sh.example          # copy to env.local.sh (gitignored) with your endpoint
  PROVISION.md                  # exact anypoint-cli + A2D commands to reproduce
```

---

## Build, test & release

Prereqs: `anypoint-cli-v4` + `anypoint-pdk-plugin`, Rust with the
`wasm32-wasip1` target, `cargo-anypoint`.

```bash
# 1) publish the definition (config-gen reads it from Exchange)
cd purpose-binding-definition
make release

# 2) generate config.rs, build to WASM, run unit tests
cd ../purpose-binding-flex
make build-asset-files
cargo build --target wasm32-wasip1 --release
cargo test --lib            # 11 pure unit tests, no Docker required

# 3) release the implementation to Exchange
make release

# (optional) Docker-based integration test — allow vs deny end to end
make test
```

**Unit test coverage** (`cargo test --lib`): allow-list resolution (per-tool
override, global fallback, not-governed), `evaluate` across allow / deny-missing /
deny-not-permitted / open-mode, and the JWT-claim reader (valid, missing claim,
non-JWT). Docker is only needed for `make test` / `make run`.

---

## Live demo

An **A2D mock MCP server** (`lookup_customer`) fronted by a **managed Flex
Gateway** instance with this policy applied
(`toolPurposes: lookup_customer → [customer_support, billing]`).

```bash
cp demo/env.local.sh.example demo/env.local.sh   # set PB_GW_URL to your governed endpoint
./demo/demo.sh
```

Expected output:

| Case | Request | Result |
|---|---|---|
| A | `initialize` (no purpose) | ✅ succeeds — not a governed method |
| B | `tools/call` + `x-processing-purpose: customer_support` | ✅ **ALLOW** — profile returned (HTTP 200) |
| C | `tools/call` + `x-processing-purpose: marketing` | ⛔ **DENY** HTTP 403 — `purpose 'marketing' is not permitted…` |
| D | `tools/call` + *no purpose header* | ⛔ **DENY** HTTP 403 — `purpose required … but none was declared` |

Full, reproducible provisioning (mock → Exchange `type=mcp` asset → MCP Flex
instance → deploy → apply policy) is in [`demo/PROVISION.md`](demo/PROVISION.md).

---

## Composition & positioning

- Bind **inbound**, and order it **after authentication** (so a JWT claim is
  available for `source: claim`) and **before** the tool executes.
- With `source: claim`, place a **JWT Validation** policy in front so the claim is
  from a verified token.
- Pairs naturally with **audit-logging** (record the declared purpose as Art. 12
  evidence) and **field-level masking / data-minimization** (project the response
  to only what the declared purpose needs).
- Because it is request-gating (no response rewrite), **MCP Support is not
  required** for this policy — the mcp endpoint proxies MCP natively. Add MCP
  Support first only if you also compose with other MCP-native policies that need
  its SSE/session framing.

---

## Design notes & gotchas

- **Fail-open on parse / non-MCP:** a body that isn't JSON-RPC, or a non-governed
  method, passes through. `failMode` governs only the *governed-call-with-bad-purpose*
  case — a misconfigured or malformed request is left for the MCP server to reject.
- **`toolPurposes` is an array of objects**, not a map. Dynamic-key objects don't
  round-trip cleanly through PDK `config-gen`; an array of `{tool, purposes}`
  generates a clean `Vec<…>` and is the reliable schema shape.
- **Deny uses HTTP 403** (a deliberate, documented deviation from the MCP "errors
  at HTTP 200" convention) so a client can route on status; the reason is still in
  the JSON-RPC error body.
- **Upstream URI is the mock surface minus `/mcp`** (trailing slash); the gateway
  maps the route onto `<upstream>mcp`. Deploy target is the gateway **resource**
  id — `api:manage` alone leaves `deployment: null` until `api:edit --withProxy` +
  `api:deploy`.

---

## Security considerations

- **Never commit a real allow-list of live purposes tied to secrets.** The config
  here is illustrative.
- **`source: claim` does not verify signatures** — it decodes the JWT payload only.
  A verified token must be guaranteed by a preceding JWT Validation policy;
  otherwise a caller can forge a purpose claim.
- Environment-specific endpoints (gateway host, mock id, instance id) live in
  `demo/env.local.sh`, which is **gitignored**; the committed docs use placeholders.
- Build artifacts (`target/`), PDK playground TLS/registration material, and
  secret files are excluded via `.gitignore`.

---

## Skills used

- **PDK** (`omni-gateway-pdk-skills`): `pdk-create-policy`, `pdk-mcp`,
  `pdk-schema-definition`, `pdk-request-headers-bodies`, `pdk-stop-execution`,
  `pdk-jwt` (reference).
- **P4A** (`p4a-skills`): `p4a-build-policy`, `p4a-verify-requirements`,
  `p4a-mcp-usage`, `p4a-test-mcp-policies-with-a2d`.
