#!/usr/bin/env bash
# Validates update loops via bootc switch and apply

cd "$(dirname "$0")"
source ./common_env.sh

echo "=== Testing Bootc Switch & Apply Sequence ==="

# Define the remote target upgrade reference image
# (it was latest at the moment of writing this)
TARGET_UPGRADE_REF="ghcr.io/amosproj/amos2026ss01-zero-downtime-linux-updates-system:commit-4b3a71d"

# Seed the target upgrade assignment in the Mock API
api "/v1/os-versions" POST "{\"commit_hash\": \"${TARGET_UPGRADE_REF}\", \"orchestrator_version\": \"0.1.0\", \"description\": \"Target GHCR Upgrade Image\"}" 201
api "/v1/os-assignments" POST '{"os_version_id": 3, "device_id": 1}' 201

echo "--- Cleaning up obsolete assignments in Cloud API ---"
# Delete id: 1 and id: 2 so that only id: 3 remains so that it for sure has to use the new aassignment here
curl -s -X DELETE -H "Authorization: Bearer ${JWT}" "${HOST_SERVER_URL}/v1/os-assignments/1"
curl -s -X DELETE -H "Authorization: Bearer ${JWT}" "${HOST_SERVER_URL}/v1/os-assignments/2"
echo "--- Obsolete assignments removed ---"

# Force orchestrator agent iteration check
echo "Restarting Orchestrator loop..."
limactl shell "${VM_NAME}" -- sudo systemctl restart orchestrator.service

echo "Awaiting Orchestrator upgrade trigger phase (allowing time for download and execution)..."
limactl shell "edge-ipc" -- sudo journalctl -u orchestrator.service -n 50 --no-pager
sleep 15

echo "Querying live bootc deployment status for staged images..."
BOOTC_STATUS_JSON=$(limactl shell "${VM_NAME}" -- sudo bootc status --json)

if echo "${BOOTC_STATUS_JSON}" | jq -e ".status.staged.image.image.image == \"${TARGET_UPGRADE_REF}\" or .status.booted.image.image.image == \"${TARGET_UPGRADE_REF}\"" > /dev/null; then
    echo -e "${GREEN}Success: Verified switch deployment! Target image matches live bootc status.${NC}"
    echo "Current bootc status output:"
    echo "${BOOTC_STATUS_JSON}" | jq .
else
    echo -e "${RED}Failure: Target image '${TARGET_UPGRADE_REF}' was not found in staged or booted status.${NC}"
    echo "Current bootc status output:"
    echo "${BOOTC_STATUS_JSON}" | jq .
    exit 1
fi