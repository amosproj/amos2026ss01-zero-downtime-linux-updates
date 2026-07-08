#!/usr/bin/env bash

cd "$(dirname "$0")"
source ./common_env.sh

echo "=== Testing Bootc Switch & Apply Sequence ==="

# Define the remote target upgrade reference image
TARGET_UPGRADE_REF="ghcr.io/amosproj/amos2026ss01-zero-downtime-linux-updates-system:commit-4b3a71d"

# Seed the target upgrade assignment in the Mock API
api "/v1/os-versions" POST "{\"commit_hash\": \"${TARGET_UPGRADE_REF}\", \"orchestrator_version\": \"0.1.0\", \"description\": \"Target GHCR Upgrade Image\"}" 201
api "/v1/os-assignments" POST '{"os_version_id": 3, "device_id": 1, "immediate": true}' 201

echo "--- Cleaning up obsolete assignments in Cloud API ---"
curl -s -X DELETE -H "Authorization: Bearer ${JWT}" "${HOST_SERVER_URL}/v1/os-assignments/1"
curl -s -X DELETE -H "Authorization: Bearer ${JWT}" "${HOST_SERVER_URL}/v1/os-assignments/2"
echo "--- Obsolete assignments removed ---"

# Force orchestrator agent iteration check
echo "Restarting Orchestrator loop..."
limactl shell "${VM_NAME}" -- sudo systemctl restart orchestrator.service

echo "Polling live bootc status for immediate upgrade execution..."

# --- Dynamic Verification Loop ---
UPGRADED=false
for i in $(seq 1 50); do
    BOOTC_STATUS_JSON=$(limactl shell "${VM_NAME}" -- sudo bootc status --json 2>/dev/null)
    
    if [ $? -eq 0 ] && [ -n "$BOOTC_STATUS_JSON" ]; then
        if echo "${BOOTC_STATUS_JSON}" | jq -e ".status.staged.image.image.image == \"${TARGET_UPGRADE_REF}\" or .status.booted.image.image.image == \"${TARGET_UPGRADE_REF}\"" > /dev/null; then
            echo -e "${GREEN}Success: Verified switch deployment! Target image matches live bootc status.${NC}"
            echo "Current bootc status output:"
            echo "${BOOTC_STATUS_JSON}" | jq .
            UPGRADED=true
            break
        fi
    fi
    sleep 2
done

if [ "$UPGRADED" = false ]; then
    echo -e "${RED}Failure: Target image '${TARGET_UPGRADE_REF}' was not found in staged or booted status after polling window.${NC}"
    echo "Last captured bootc status:"
    limactl shell "${VM_NAME}" -- sudo bootc status --json | jq .
    exit 1
fi