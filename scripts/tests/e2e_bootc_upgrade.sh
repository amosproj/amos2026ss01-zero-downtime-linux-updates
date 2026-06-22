#!/usr/bin/env bash
# Validates update loops via bootc switch and apply

cd "$(dirname "$0")"
source ./common_env.sh

echo "=== Testing Bootc Switch & Apply Sequence ==="

# Define the remote target upgrade reference image
# (it was latest at the moment of writing this)
TARGET_UPGRADE_REF="ghcr.io/amosproj/amos2026ss01-zero-downtime-linux-updates-system:commit-4b3a71d"

echo "Preparing local upgrade target image inside the VM..."
# Pre-pull the image from GHCR into root's containers-storage backend inside the VM
limactl shell "${VM_NAME}" -- sudo podman pull "${TARGET_UPGRADE_REF}"

# Seed the target upgrade assignment in the Mock API
api "/v1/os-versions" POST "{\"commit_hash\": \"${TARGET_UPGRADE_REF}\", \"orchestrator_version\": \"0.1.0\", \"description\": \"Target GHCR Upgrade Image\"}" 201
api "/v1/os-assignments" POST '{"os_version_id": 3, "device_id": 1}' 201

echo "--- Cleaning up obsolete assignments in Cloud API ---"
# Delete id: 1 and id: 2 so that only id: 3 remains so that it for sure has to use the new aassignment here
curl -s -X DELETE -H "Authorization: Bearer ${JWT}" "${HOST_SERVER_URL}/v1/os-assignments/1"
curl -s -X DELETE -H "Authorization: Bearer ${JWT}" "${HOST_SERVER_URL}/v1/os-assignments/2"
echo "--- Obsolete assignments removed ---"

echo "--- DEBUG: Current OS Assignments in Cloud API ---"
curl -s -H "Authorization: Bearer ${JWT}" \
    "http://127.0.0.1:${PORT}/v1/os-assignments?device_uuid=${DEVICE_UUID}" | jq '.'
echo "---------------------------------------------------"

# Force orchestrator agent iteration check
echo "Restarting Orchestrator loop..."
limactl shell "${VM_NAME}" -- sudo systemctl restart orchestrator.service

echo "Awaiting Orchestrator upgrade trigger phase (allowing time for download and execution)..."
limactl shell "edge-ipc" -- sudo journalctl -u orchestrator.service -n 50 --no-pager
sleep 15

# --- VERIFY THE ACTUAL SWITCH EFFECT IN BOOTC ---
echo "Querying live bootc deployment status for staged images..."
BOOTC_STATUS_RAW=$(limactl shell "${VM_NAME}" -- sudo bootc status)

if echo "${BOOTC_STATUS_RAW}" | grep -A 5 "staged" | grep -q "${TARGET_UPGRADE_REF}"; then
    echo -e "${GREEN}Success: Verified switch deployment! Target image is staged in bootc status.${NC}"
else
    echo -e "${RED}Failure: Target image '${TARGET_UPGRADE_REF}' was not found in bootc's staged status.${NC}"
    echo "Current bootc status output:"
    echo "${BOOTC_STATUS_RAW}"
    exit 1
fi