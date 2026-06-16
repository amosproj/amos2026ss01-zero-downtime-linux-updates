#!/usr/bin/env bash
# Fills the active api mock server database

set -euo pipefail

cd "$(dirname "$0")/.."

source ./tests/common_env.sh

echo "=== Filling Mock Cloud Database ==="


api "/v1/tenants" POST '{ "name": "Weber-Lager", "description": "Automated Testing Tenant" }' 201
api "/v1/devices" POST "{\"uuid\": \"${DEVICE_UUID}\", \"hostname\": \"edge-ipc\", \"tenant_id\": 1}" 201
api "/v1/os-versions" POST '{ "commit_hash": "092599a804d5169ae2a0a306bcb4b213b7646d28", "orchestrator_version": "0.1.0", "description": "Target Update Commit" }' 201
api "/v1/os-assignments" POST '{ "os_version_id": 1, "device_id": 1 }' 201

echo -e "${GREEN}Database successfully initialized with testing data.${NC}"

# Inject target authentication tokens directly into the VM runtime environment
limactl shell "${VM_NAME}" -- sudo bash -c "grep -q 'auth_token' /etc/amos/config.toml || echo 'auth_token = \"${JWT}\"' >> /etc/amos/config.toml"

echo -e "${GREEN}Environment is fully provisioned and primed.${NC}"