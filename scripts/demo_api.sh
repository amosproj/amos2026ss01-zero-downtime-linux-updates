#!/usr/bin/env bash
# Convenience wrapper for sending authenticated API commands to the mock cloud
# server started by `make demo-server` (scripts/demo_run.sh). It attaches the dev JWT
# and pretty-prints the JSON response, so you can drive the demo without
# retyping curl + the auth header every time.
#
# Usage:
#   ./scripts/demo_api.sh GET  /v1/devices
#   ./scripts/demo_api.sh GET  /v1/devices/1/summary
#   ./scripts/demo_api.sh POST /v1/applications '{ "name": "hello", "description": "x" }'
#
# The path is used as-is (no /v1 auto-prefixing), so pass the full path,
# e.g. /run1/v1/devices.

set -euo pipefail

cd "$(dirname "$0")"
source ./tests/common_env.sh

PORT=80
HOST_SERVER_URL="${DEMO_API_URL:-http://float-172-017-069-035.cc.rrze.net:${PORT}}"

method="${1:-}"
path="${2:-}"
body="${3:-}"

if [ -z "$method" ] || [ -z "$path" ]; then
    echo "usage: demo_api.sh METHOD PATH [JSON_BODY]" >&2
    echo "example: demo_api.sh POST /v1/applications '{ \"name\": \"hello\", \"description\": \"x\" }'" >&2
    exit 2
fi

case "$path" in
    /*) ;;
    *)  path="/${path}" ;;
esac

url="${HOST_SERVER_URL}${path}"
echo ">>> ${method} ${url}" >&2

if [ -n "$body" ]; then
    curl -sS -X "$method" "$url" \
        -H "Content-Type: application/json" \
        -H "Authorization: Bearer ${JWT}" \
        -d "$body"
else
    curl -sS -X "$method" "$url" \
        -H "Authorization: Bearer ${JWT}"
fi | { jq . 2>/dev/null || cat; }
echo
