#!/usr/bin/env bash
set -eu

readonly api_base_path="http://localhost:8080/v1"
readonly script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly server_dir="$(cd "$script_dir/../api-mock-server" && pwd)"

failed_tests=()
server_pid=

# With secret
# readonly jwt='eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6Ik1hcmMgV2ViZXIiLCJleHAiOjMzMzM3NjAxMDg2fQ.fMYxL8k4-svFDQ4ZAZLlFtEDkEXVRALD9qj5YBRalT4'
# With eddsa
readonly jwt='eyJhbGciOiJFZERTQSIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6Ik1hcmMgV2ViZXIiLCJleHAiOjMzMzM3NjAxMDg2fQ.ojUwb9caLF_Ao0wT6DDChYam79ZqnlRPED0PA6XG5r42gFqKWSbRkDb12CuiLczwp5PpxOxXVCaLwLqFnQ13Dw'

api() {
    local path="$1"
    shift
    local method="${1:-GET}"
    shift || true
    local body="${1:-}"
    shift || true
    local expected_code="${1:-200}"
    shift || true

    local url="${api_base_path}/${path#/}"
    local response actual_code
    if [ -n "$body" ]; then
        response=$( (
            set -x
            curl -sS -D - -X "$method" "$url" -H "Content-Type: application/json" -H "Authorization: Bearer ${jwt}" -d "$body" -w "\n%{http_code}"
        ))
    else
        response=$( (
            set -x
            curl -sS -D - -X "$method" "$url" -H "Content-Type: application/json" -H "Authorization: Bearer ${jwt}" -w "\n%{http_code}"
        ))
    fi
    actual_code=$(printf '%s\n' "$response" | tail -n1)
    printf '%s\n' "$response" | head -n -1
    echo -e "\n---------------"

    if [ "$actual_code" != "$expected_code" ]; then
        failed_tests+=("FAIL [$method $path]: expected $expected_code, got $actual_code")
    fi
}

print_results() {
    echo ""
    if [ ${#failed_tests[@]} -eq 0 ]; then
        echo "All tests passed."
    else
        echo "Failed tests:"
        for msg in "${failed_tests[@]}"; do
            echo "  $msg"
        done
        return 1
    fi
}

cleanup() {
    # Kill first — print_results may exit 1, so kill must happen before it.
    # Kill the entire process group so cargo's child (the server binary) is also
    # terminated; just killing cargo leaves the binary orphaned on the port.
    if [ -n "$server_pid" ]; then
        echo "Stopping server (pid $server_pid)..."
        kill -- "-$server_pid" 2>/dev/null || true
        wait "$server_pid" 2>/dev/null || true
    fi
    print_results
}
trap cleanup EXIT

# Start the server in its own process group (set -m) so that kill -- -$pid
# reaches both cargo and the server binary it spawns.
echo "Starting api-mock-server..."
set -m
APP_DATABASE_URL="sqlite::memory:" cargo run --manifest-path "$server_dir/Cargo.toml" -- -dd &
server_pid=$!
set +m

# Wait for the server to be ready
echo "Waiting for server to be ready..."
for i in $(seq 1 30); do
    if curl -sS -o /dev/null "http://localhost:8080/v1/tenants" 2>/dev/null; then
        echo "Server is ready."
        break
    fi
    if [ "$i" -eq 30 ]; then
        echo "Server did not become ready in time." >&2
        exit 1
    fi
    sleep 1
done

# Create random strings:
# uuidv4: `uuidgen`
# sha1: `head -c 32 /dev/urandom | sha1sum`

api "/tenants" POST '{ "name": "Weber-Lager", "description": "Meta-Ort für initialisierte unverschickte Geräte" }' 201
api "/tenants" POST '{ "name": "Kaufland-Fabrik-Erlangen", "description": "Stammkunde in Deutschland" }' 201
api "/tenants" POST '{ "name": "7-Eleven-Fabrik-Tokyo", "description": "Zentrale Stelle in Chiyoda für Tokyo" }' 201
api "/tenants" POST '{ "name": "Foodland-Fabrik-Bangkok", "description": "Hauptlagerort in Bangkok" }' 201
api "/devices" POST '{ "uuid": "8b722f94-6852-42cf-9722-98446499a457", "hostname": "x38974", "tenant_id": 1 }' 201

api "/os-versions" POST '{ "commit_hash": "092599a804d5169ae2a0a306bcb4b213b7646d28", "orchestrator_version": "0.1.0", "description": "First stable release, tested intensively" }' 201

api "/os-assignments" POST '{ "os_version_id": 1, "device_id": 1 }' 201

# api "/os-assignments?device_uuid=8b722f94-6852-42cf-9722-98446499a457" # works
# api "/os-assignments?device_uuid=abc-123" # fails

# --- reported-os-assignments ---

api "/reported-os-assignments" POST '{ "os_version_id": 1, "device_id": 1 }' 201

api "/reported-os-assignments?device_uuid=8b722f94-6852-42cf-9722-98446499a457" POST '{ "os_version_id": 1 }' 201

api "/reported-os-assignments?device_uuid=00000000-0000-0000-0000-000000000000" POST '{ "os_version_id": 1 }' 404
