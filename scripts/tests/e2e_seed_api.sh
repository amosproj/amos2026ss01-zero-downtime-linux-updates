#!/usr/bin/env bash
# Fills the active api mock server database

set -euo pipefail

cd "$(dirname "$0")/.."
source ./tests/common_env.sh

echo "=== Filling Mock Cloud Database ==="

api "/v1/tenants" POST '{ "name": "Weber-Lager", "description": "Automated Testing Tenant" }' 201
api "/v1/devices" POST "{\"uuid\": \"${DEVICE_UUID}\", \"serial_number\": \"edge-ipc\", \"tenant_id\": 1}" 201

# Dynamically fetch the current running OSTree checksum from the VM
echo "Extracting dynamic baseline checksum from VM for database seeding..."
DYNAMIC_CHECKSUM=$(limactl shell "${VM_NAME}" -- sudo bootc status --json | jq -r '.status.booted.ostree.checksum')

if [ -z "${DYNAMIC_CHECKSUM}" ] || [ "${DYNAMIC_CHECKSUM}" == "null" ]; then
    echo -e "${RED}Critical initialization failure: Could not extract dynamic checksum from VM.${NC}"
    exit 1
fi
echo "Seeding API with Baseline Version: ${DYNAMIC_CHECKSUM}"

# Inject the dynamic checksum into the JSON payload
api "/v1/os-versions" POST "{\"commit_hash\": \"${DYNAMIC_CHECKSUM}\", \"orchestrator_version\": \"0.1.0\", \"description\": \"Dynamic Base Baseline\"}" 201
api "/v1/os-assignments" POST '{ "os_version_id": 1, "device_id": 1 }' 201

echo -e "${GREEN}Database successfully initialized with testing data.${NC}"

echo -e "${GREEN}Environment is fully provisioned and primed.${NC}"
