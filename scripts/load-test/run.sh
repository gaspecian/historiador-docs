#!/usr/bin/env bash
# Sprint 10 item #7 — run the MCP load test.
#
# Prerequisites:
#   1. Chronik + Postgres + api + mcp all running (docker compose up -d).
#   2. Chunks seeded:
#        cargo run --release -p historiador_api --bin load-test-seed
#   3. `oha` installed:
#        cargo install oha        # or brew install oha
#   4. MCP_BEARER_TOKEN exported (copy from the admin dashboard or .env).
#
# Target: p95 < 2s for 1,000 queries against 10,000 seeded chunks.
#
# The script fires 100 requests per query phrase across 10 phrases for
# a total of 1,000 requests. Output is a short one-line summary per
# phrase plus a combined table written to docs/performance.md if
# --emit-report is passed.

set -euo pipefail

MCP_URL="${MCP_URL:-http://localhost:3002/mcp}"
TOKEN="${MCP_BEARER_TOKEN:-}"
TOTAL_PER_PHRASE="${TOTAL_PER_PHRASE:-100}"
CONCURRENCY="${CONCURRENCY:-10}"
EMIT_REPORT=0

for arg in "$@"; do
    case "$arg" in
        --emit-report) EMIT_REPORT=1 ;;
        -h|--help)
            sed -n '2,20p' "$0"
            exit 0
            ;;
        *)
            echo "unknown flag: $arg" >&2
            exit 2
            ;;
    esac
done

if [ -z "$TOKEN" ]; then
    echo "MCP_BEARER_TOKEN is required — export it before running" >&2
    exit 2
fi
if ! command -v oha >/dev/null 2>&1; then
    echo "oha not found — install with: cargo install oha" >&2
    exit 2
fi
if ! command -v jq >/dev/null 2>&1; then
    echo "jq not found — install it (apt/brew) to parse oha output" >&2
    exit 2
fi

PHRASES=(
    "employee onboarding process"
    "authentication and session tokens"
    "vector search retrieval"
    "markdown chunker at heading boundaries"
    "multilingual BCP 47 language tags"
    "MCP JSON-RPC protocol"
    "Postgres role separation"
    "Chronik Stream event topics"
    "page version history restore"
    "ollama embedding provider"
)

TMP="$(mktemp -d -t mcp-load-XXXX)"
trap 'rm -rf "$TMP"' EXIT

printf '%-45s %10s %10s %10s\n' "query" "p50_ms" "p95_ms" "p99_ms"
printf '%-45s %10s %10s %10s\n' "---------------------------------------------" "--------" "--------" "--------"

TOTAL_P50=0
TOTAL_P95=0
TOTAL_P99=0
PHRASE_COUNT=${#PHRASES[@]}

for i in "${!PHRASES[@]}"; do
    phrase="${PHRASES[$i]}"
    # Build a JSON-RPC tools/call payload. `id` rotates so concurrent
    # in-flight requests do not collide on the client.
    body="$(jq -n --arg q "$phrase" --argjson id "$i" \
        '{jsonrpc: "2.0", id: $id, method: "tools/call", params: {name: "query", arguments: {query: $q, top_k: 5}}}')"

    out="$TMP/phrase_$i.json"
    oha --no-tui -q -n "$TOTAL_PER_PHRASE" -c "$CONCURRENCY" \
        -m POST \
        -H "Authorization: Bearer $TOKEN" \
        -H "Content-Type: application/json" \
        -d "$body" \
        --output-format json \
        "$MCP_URL" > "$out"

    # oha emits times in seconds; convert to ms.
    p50_ms="$(jq '.latencyPercentiles.p50 * 1000 | floor' "$out")"
    p95_ms="$(jq '.latencyPercentiles.p95 * 1000 | floor' "$out")"
    p99_ms="$(jq '.latencyPercentiles.p99 * 1000 | floor' "$out")"

    printf '%-45s %10s %10s %10s\n' "$phrase" "$p50_ms" "$p95_ms" "$p99_ms"

    TOTAL_P50=$((TOTAL_P50 + p50_ms))
    TOTAL_P95=$((TOTAL_P95 + p95_ms))
    TOTAL_P99=$((TOTAL_P99 + p99_ms))
done

AVG_P50=$((TOTAL_P50 / PHRASE_COUNT))
AVG_P95=$((TOTAL_P95 / PHRASE_COUNT))
AVG_P99=$((TOTAL_P99 / PHRASE_COUNT))

echo
echo "---- averaged across ${PHRASE_COUNT} phrases × ${TOTAL_PER_PHRASE} requests = $((PHRASE_COUNT * TOTAL_PER_PHRASE)) total ----"
printf 'p50: %sms   p95: %sms   p99: %sms\n' "$AVG_P50" "$AVG_P95" "$AVG_P99"

if [ "$EMIT_REPORT" -eq 1 ]; then
    # Append a dated section to docs/performance.md. Operators can then
    # edit the narrative in place.
    REPORT="docs/performance.md"
    DATE="$(date -u +%Y-%m-%d)"
    {
        echo
        echo "## Run on $DATE"
        echo
        echo "- MCP URL: \`$MCP_URL\`"
        echo "- Concurrency: $CONCURRENCY"
        echo "- Requests per phrase: $TOTAL_PER_PHRASE"
        echo "- Phrases: $PHRASE_COUNT"
        echo "- **p50: ${AVG_P50}ms, p95: ${AVG_P95}ms, p99: ${AVG_P99}ms**"
        echo
    } >> "$REPORT"
    echo "appended summary to $REPORT"
fi

# Exit non-zero if p95 exceeds the v1.0 target so CI can fail the run.
if [ "$AVG_P95" -gt 2000 ]; then
    echo "p95 exceeds v1.0 target of 2000ms — remediation required" >&2
    exit 1
fi
