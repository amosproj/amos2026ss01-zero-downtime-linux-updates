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
# The path may be given with or without the leading /v1.

set -euo pipefail

cd "$(dirname "$0")"
source ./tests/common_env.sh

method="${1:-}"
path="${2:-}"
body="${3:-}"

if [ -z "$method" ] || [ -z "$path" ]; then
    echo "usage: demo_api.sh METHOD PATH [JSON_BODY]" >&2
    echo "example: demo_api.sh POST /v1/applications '{ \"name\": \"hello\", \"description\": \"x\" }'" >&2
    exit 2
fi

# Accept paths with or without the /v1 prefix, and with or without a leading /.
case "$path" in
    /v1/*) ;;
    /*)    path="/v1${path}" ;;
    *)     path="/v1/${path}" ;;
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
