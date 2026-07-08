#!/usr/bin/env bash
set -eu

readonly api_base_path="http://localhost:8080/v1"
readonly script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly server_dir="$(cd "$script_dir/../api-server" && pwd)"
readonly devcontainer_dir="$(cd "$script_dir/../.devcontainer" && pwd)"

readonly timescale_container="amos-test-timescaledb"
readonly timescale_port=55433
readonly timescale_url="postgres://app:4M0S@127.0.0.1:${timescale_port}/amos_timeseries"

failed_tests=()
server_pid=

# JWT signed with the default dev key, valid 1000 years
readonly jwt='eyJhbGciOiJSUzUxMiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6Ik1hcmMgV2ViYmVyIiwiZXhwIjozMzMzNzg2MDc1Nn0.YLzANsYJj5TmCAURvMyUQSSeGk6fa8xrJhrbSrm999hMVxeYqTtT2c62dT7Ast9bdHENHWAPZD7OYWsOK2sCX-jqYfNFgmAmYxCtLaXMCVgIvqzOWf9miV8F5Zd8OaSnoaWbA7iXsICJ_kBYCP6zFdRQUoO-Evok4vtzH6Y5M1LyJtsy65NIpkpQt6DAZqf0s7818mrJdqpLp_L_1vqPq9QOrMen28lv_RNjWl5x9_lGhfw15TbGhfrE5mvmzsq6RW6M5Eun3CVGWXERqNzOqdVHo13BtmyRxLbJa8kP0r0qPubMfQf-bpAIVxG6oA5xbjytiEKQ8vfl1up6XBn429N_039-exEfv8EdZ35AjqLpLaSA4BM0RFurqZMse4ELJmNRPQLVMfrBDTf0yLB3USi0su3tFZRXQ6ND7cLpqL6PUYL0KrJZUiMwD8ZMSDBO7Rilh2thkhYp0EfBncIi5lI1gVlN5qSC51NJeDBRFPYnhH_-gwxecn1WzVILpiNki0E8euOpSTXgS2FNxlHhPfBevPodoBn8j-Vu0U9-8xmfqxZirGankWz4d00rthBn_B0IFKk0WFy742TW_Qs9NdAL9UnGJGwqYv88MtGo6vgfTwdE9WASkq4ubJ8GCvFmooKb9FrMGz_-9pS2RWRgO_kT_1PSD4bTMHQIMhC1eXs'

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

    echo "Stopping TimescaleDB container..."
    podman rm -f "$timescale_container" >/dev/null 2>&1 || true

    print_results
}
trap cleanup EXIT

# Start a throwaway TimescaleDB container for this run.
podman rm -f "$timescale_container" >/dev/null 2>&1 || true
echo "Starting TimescaleDB container..."
podman run -d --name "$timescale_container" \
    -e POSTGRES_PASSWORD=dummy \
    -p "127.0.0.1:${timescale_port}:5432" \
    -v "$devcontainer_dir/20_setup_timescale_db.sh:/docker-entrypoint-initdb.d/20_setup_timescale_db.sh:ro,Z" \
    docker.io/timescale/timescaledb:latest-pg18 >/dev/null

echo "Waiting for TimescaleDB to be ready..."
for i in $(seq 1 60); do
    if podman exec "$timescale_container" pg_isready -U postgres >/dev/null 2>&1; then
        echo "TimescaleDB is ready."
        break
    fi
    if [ "$i" -eq 60 ]; then
        echo "TimescaleDB did not become ready in time." >&2
        exit 1
    fi
    sleep 1
done

# Start the server in its own process group (set -m) so that kill -- -$pid
# reaches both cargo and the server binary it spawns.
echo "Starting api-server..."
set -m
APP_DATABASE_URL="sqlite::memory:" APP_TIMESCALE_DATABASE_URL="$timescale_url" cargo run --manifest-path "$server_dir/Cargo.toml" -- -dd &
server_pid=$!
set +m

# Wait for the server to be ready
echo "Waiting for server to be ready..."
for i in $(seq 1 60); do
    if curl -sS -o /dev/null "http://localhost:8080/v1/tenants" 2>/dev/null; then
        echo "Server is ready."
        break
    fi
    if [ "$i" -eq 60 ]; then
        echo "Server did not become ready in time." >&2
        exit 1
    fi
    sleep 1
done

# --- set up a tenant, device and application to log against ---

api "/tenants" POST '{ "name": "Weber-Lager", "description": "Meta-Ort für initialisierte unverschickte Geräte" }' 201

readonly device_uuid="8b722f94-6852-42cf-9722-98446499a457"
api "/devices" POST "{ \"uuid\": \"${device_uuid}\", \"serial_number\": \"x38974\", \"tenant_id\": 1 }" 201

api "/applications" POST '{ "name": "amos-orchestrator", "description": "Orchestrator agent" }' 201

# --- /logs/devices ---

api "/logs/devices?device_uuid=${device_uuid}" POST '{
    "entries": [
        { "time": "2026-06-12T10:00:00Z", "level": "info", "message": "Orchestrator started", "source": "amos-orchestrator" },
        { "time": null, "level": "warn", "message": "Disk usage above 80%", "source": null }
    ]
}' 201

api "/logs/devices?device_uuid=${device_uuid}" POST '{ "entries": [] }' 422

api "/logs/devices" POST '{ "entries": [ { "time": null, "level": "info", "message": "no device_uuid given", "source": null } ] }' 422

api "/logs/devices?device_uuid=00000000-0000-0000-0000-000000000000" POST '{ "entries": [ { "time": null, "level": "info", "message": "unknown device", "source": null } ] }' 404

# --- GET /logs/devices (historic query) ---

api "/logs/devices" GET '' 200

api "/logs/devices?device_id=1&level=warn&from=2026-06-12T00:00:00Z&to=2026-06-12T23:59:59Z" GET '' 200

# --- /logs/applications ---

api "/logs/applications?device_uuid=${device_uuid}" POST '{
    "application_id": 1,
    "entries": [
        { "time": null, "level": "error", "message": "Connection refused", "source": "amos-orchestrator" }
    ]
}' 201

# --- GET /logs/applications (historic query) ---

api "/logs/applications" GET '' 200

api "/logs/applications?device_id=1&application_id=1&level=error" GET '' 200

# --- /logs/stream (SSE) ---

echo "--- GET /logs/stream?level=warn ---"
stream_file=$(mktemp)
(timeout 5 curl -sS -N -H "Authorization: Bearer ${jwt}" "${api_base_path}/logs/stream?level=warn" >"$stream_file") &
stream_pid=$!
sleep 1

api "/logs/devices?device_uuid=${device_uuid}" POST '{
    "entries": [ { "time": null, "level": "debug", "message": "debug log, should be filtered out", "source": null } ]
}' 201

api "/logs/devices?device_uuid=${device_uuid}" POST '{
    "entries": [ { "time": null, "level": "error", "message": "error log, should be streamed", "source": null } ]
}' 201

sleep 1
wait "$stream_pid" 2>/dev/null || true

echo "Stream contents:"
cat "$stream_file"
echo -e "\n---------------"

if grep -q "error log, should be streamed" "$stream_file" && ! grep -q "debug log, should be filtered out" "$stream_file"; then
    echo "SSE stream filtering OK"
else
    failed_tests+=("FAIL [GET /logs/stream]: did not receive expected events with level=warn filter")
fi
rm -f "$stream_file"
