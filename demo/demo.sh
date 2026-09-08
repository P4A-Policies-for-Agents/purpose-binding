#!/usr/bin/env bash
# Live demo: Purpose Binding policy.
#
# Same MCP tools/call, sent with different (or no) declared processing purpose,
# through the Flex Gateway with the Purpose Binding policy applied. A permitted
# purpose passes; a disallowed or missing purpose is rejected at the gateway
# (HTTP 403 + JSON-RPC error) before the tool is ever invoked.
#
# Real endpoints are environment-specific — keep them out of the repo.
# Put them in demo/env.local.sh (gitignored); see env.local.sh.example.

set -uo pipefail
DIR="$(cd "$(dirname "$0")" && pwd)"
[ -f "$DIR/env.local.sh" ] && . "$DIR/env.local.sh"
GW="${PB_GW_URL:?Set PB_GW_URL (e.g. via demo/env.local.sh — see env.local.sh.example)}"

HDR=(-H 'Content-Type: application/json' -H 'Accept: application/json, text/event-stream' -H 'Accept-Encoding: identity')
CALL='{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"lookup_customer","arguments":{"customerId":"C-1001"}}}'

show() { sed -n 's/^data: //p; /^{/p' | head -1; }

echo "=================================================================="
echo " A) initialize — no purpose required (not a governed method)"
echo "=================================================================="
curl -sS --max-time 25 "${HDR[@]}" -X POST "$GW" \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"demo","version":"1"}}}' \
  -w " [HTTP %{http_code}]\n" | show

echo
echo "=================================================================="
echo " B) tools/call  purpose = customer_support   -> ALLOW"
echo "=================================================================="
curl -sS --max-time 25 "${HDR[@]}" -H 'x-processing-purpose: customer_support' -X POST "$GW" -d "$CALL" -w " [HTTP %{http_code}]\n" | show

echo
echo "=================================================================="
echo " C) tools/call  purpose = marketing          -> DENY (403)"
echo "=================================================================="
curl -sS --max-time 25 "${HDR[@]}" -H 'x-processing-purpose: marketing' -X POST "$GW" -d "$CALL" -w " [HTTP %{http_code}]\n"

echo
echo "=================================================================="
echo " D) tools/call  (no purpose header)          -> DENY (403)"
echo "=================================================================="
curl -sS --max-time 25 "${HDR[@]}" -X POST "$GW" -d "$CALL" -w " [HTTP %{http_code}]\n"
