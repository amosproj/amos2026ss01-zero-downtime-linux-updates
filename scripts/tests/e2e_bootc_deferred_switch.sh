#!/usr/bin/env bash

cd "$(dirname "$0")"
source ./common_env.sh

echo "=== Testing Deferred Bootc Switch ==="

echo " --- Configuring 30-second deferred timer for testing ---"
limactl shell "${VM_NAME}" -- sudo bash -c "grep -q 'deferred_switch_timer_secs' /etc/amos/config.toml || echo 'deferred_switch_timer_secs = 30' >> /etc/amos/config.toml"

# Restart Orchestrator so it picks up the new config
limactl shell "${VM_NAME}" -- sudo systemctl stop orchestrator.service
sleep 2

# Setup the deferred target assignment
TARGET_UPGRADE_REF="ghcr.io/amosproj/amos2026ss01-zero-downtime-linux-updates-system:switch-2-tag"

# Clean up old assignments first
# TODO: I think it's better to test whether that's necessary too
curl -s -X DELETE -H "Authorization: Bearer ${JWT}" "${HOST_SERVER_URL}/v1/os-assignments/1"
curl -s -X DELETE -H "Authorization: Bearer ${JWT}" "${HOST_SERVER_URL}/v1/os-assignments/2"
curl -s -X DELETE -H "Authorization: Bearer ${JWT}" "${HOST_SERVER_URL}/v1/os-assignments/3"

# Setup the deferred target assignment
api "/v1/os-versions" POST "{\"commit_hash\": \"${TARGET_UPGRADE_REF}\", \"orchestrator_version\": \"0.1.1\", \"description\": \"Deferred Target Upgrade Image\"}" 201
api "/v1/os-assignments" POST '{"os_version_id": 4, "device_id": 1, "immediate": false}' 201

# Restart Orchestrator so it picks up the new config
limactl shell "${VM_NAME}" -- sudo systemctl start orchestrator.service
sleep 2


echo "--- Waiting for Orchestrator to detect, download, stage, and boot the update ---"

APPLIED=false
for i in $(seq 1 45); do
    BOOTC_STATUS_JSON=$(limactl shell "${VM_NAME}" -- sudo bootc status --json 2>/dev/null)
    
    # limactl may temporarily fail if the host agent triggers an immediate reboot/restart sequence
    if [ $? -eq 0 ] && [ -n "$BOOTC_STATUS_JSON" ]; then
        if echo "${BOOTC_STATUS_JSON}" | jq -e ".status.booted.image.image.image == \"${TARGET_UPGRADE_REF}\"" > /dev/null; then
            echo -e "${GREEN}Success: Verified state change! Update is fully booted and applied.${NC}"
            echo "${BOOTC_STATUS_JSON}" | jq .
            APPLIED=true
            break
        fi
    fi
    sleep 2
done

if [ "$APPLIED" = false ]; then
    echo -e "${RED}Failure: Target upgrade was not found in the 'booted' state within the timeout.${NC}"
    exit 1
fi