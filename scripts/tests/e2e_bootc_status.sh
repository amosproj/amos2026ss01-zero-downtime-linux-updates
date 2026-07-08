#!/usr/bin/env bash
# Validates bootc status reporting and up-to-date evaluation paths

cd "$(dirname "$0")"
source ./common_env.sh

echo "=== Testing Bootc Status Evaluation ==="

# Gather current state using JSON parsing via jq
echo "Fetching active booted digest from VM..."
BOOTED_DIGEST=$(limactl shell "${VM_NAME}" -- sudo bootc status --json | jq -r '.status.booted.ostree.checksum')

# Validation: ensure we didn't get an empty string or 'null' from jq
if [ -z "${BOOTED_DIGEST}" ] || [ "${BOOTED_DIGEST}" == "null" ]; then
    echo -e "${RED}Failed to extract live booted digest from VM via JSON!${NC}"
    exit 1
fi
echo "Live VM is running deployment digest: ${BOOTED_DIGEST}"

# Setup the target assignment in the API
api "/v1/os-versions" POST "{\"commit_hash\": \"${BOOTED_DIGEST}\", \"orchestrator_version\": \"0.1.0\", \"description\": \"Current Base Baseline\"}" 201
api "/v1/os-assignments" POST '{"os_version_id": 2, "device_id": 1}' 201

# Trigger the agent loop
echo "Restarting Orchestrator loop to trigger check-in..."
limactl shell "${VM_NAME}" -- sudo systemctl restart orchestrator.service
sleep 5

echo "Verifying that the device reported its OS assignment upstream..."
REPORTED_STATUS=$(curl -sS -H "Authorization: Bearer ${JWT}" "${HOST_SERVER_URL}/v1/reported-os-assignments?device_uuid=${DEVICE_UUID}")

# Use jq to extract the exact os_version_id from the first record in the data array
REPORTED_VERSION_ID=$(echo "${REPORTED_STATUS}" | jq '.data[0].os_version_id // empty')

echo "API server reports active device os_version_id is: '${REPORTED_VERSION_ID}'"

# Validate that a version was reported and that it maps to 1
if [ "${REPORTED_VERSION_ID}" = "1" ]; then
    echo -e "${GREEN}Success: Device correctly reported its OS assignment state (mapped to baseline ID 1).${NC}"
else
    echo -e "${RED}Failure: Device did not report version 1. Received: '${REPORTED_VERSION_ID}'${NC}"
    echo "Full response payload: ${REPORTED_STATUS}"
    exit 1
fi