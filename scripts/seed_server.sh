#!/usr/bin/env bash
set -eu

readonly api_base_path="http://localhost:8080/v1"

api() {
    local path="$1"; shift
    local method="${1:-GET}"; shift || true
    local body="${1:-}"; shift || true

    local url="${api_base_path}/${path#/}"
    if [ -n "$body" ]; then
        ( set -x; curl -i -sS -X "$method" "$url" -H "Content-Type: application/json" -d "$body" )
    else
        ( set -x; curl -i -sS -X "$method" "$url" -H "Content-Type: application/json" )
    fi
    echo -e "\n---------------"
}

# Create random strings:
# uuidv4: `uuidgen`
# sha1: `head -c 32 /dev/urandom | sha1sum`

api "/tenants" POST '{ "id": 0, "name": "Weber-Lager", "description": "Meta-Ort für initialisierte unverschickte Geräte" }'
api "/tenants" POST '{ "id": 0, "name": "Kaufland-Fabrik-Erlangen", "description": "Stammkunde in Deutschland" }'
api "/tenants" POST '{ "id": 0, "name": "7-Eleven-Fabrik-Tokyo", "description": "Zentrale Stelle in Chiyoda für Tokyo" }'
api "/tenants" POST '{ "id": 0, "name": "Foodland-Fabrik-Bangkok", "description": "Hauptlagerort in Bangkok" }'
api "/devices" POST '{ "id": 0, "uuid": "8b722f94-6852-42cf-9722-98446499a457", "hostname": "x38974", "tenant_id": 1 }'

api "/os-versions" POST '{ "id": 0, "commit_hash": "092599a804d5169ae2a0a306bcb4b213b7646d28", "orchestrator_version": "0.1.0", "description": "First stable release, tested intensively" }'

api "/os-assignments" POST '{ "id": 0, "os_version_id": 1, "device_id": 1 }'

# api "/os-assignments?device_uuid=8b722f94-6852-42cf-9722-98446499a457" # works
# api "/os-assignments?device_uuid=abc-123" # fails

# --- reported-os-assignments ---

# Create using explicit device_id in body (expects 201)
api "/reported-os-assignments" POST '{ "id": 0, "os_version_id": 1, "device_id": 1, "updated_at": "1970-01-01T00:00:00Z" }'

# Create using device_uuid query param to resolve device_id (expects 201)
api "/reported-os-assignments?device_uuid=8b722f94-6852-42cf-9722-98446499a457" POST '{ "id": 0, "os_version_id": 1, "device_id": 0, "updated_at": "1970-01-01T00:00:00Z" }'

# Create with non-existent device_uuid (expects 404)
api "/reported-os-assignments?device_uuid=00000000-0000-0000-0000-000000000000" POST '{ "id": 0, "os_version_id": 1, "device_id": 0, "updated_at": "1970-01-01T00:00:00Z" }'
