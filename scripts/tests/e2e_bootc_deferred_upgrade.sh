#!/usr/bin/env bash
# Tests deferred update loop: download-only, staging, and timer-based application

cd "$(dirname "$0")"
source ./common_env.sh

echo "=== Testing Deferred Bootc Upgrade ==="

# Inject a short 5-second deferred timer into the orchestrator config
echo "Configuring 5-second deferred timer for testing..."
limactl shell "${VM_NAME}" -- sudo bash -c "grep -q 'deferred_update_timer_secs' /etc/amos/config.toml || echo 'deferred_update_timer_secs = 5' >> /etc/amos/config.toml"

# Restart Orchestrator so it picks up the new config
limactl shell "${VM_NAME}" -- sudo systemctl restart orchestrator.service
sleep 2

# Setup the deferred target assignment (immediate = false)
TARGET_UPGRADE_REF="ghcr.io/amosproj/amos2026ss01-zero-downtime-linux-updates-system:main"

api "/v1/os-versions" POST "{\"commit_hash\": \"${TARGET_UPGRADE_REF}\", \"orchestrator_version\": \"0.1.1\", \"description\": \"Deferred Target Upgrade Image\"}" 201
api "/v1/os-assignments" POST '{"os_version_id": 4, "device_id": 1, "immediate": false}' 201

# Clean up previous assignments so it picks up ID 4 (adjust IDs based on your DB state)
# TODO: This should not be necessary imo
curl -s -X DELETE -H "Authorization: Bearer ${JWT}" "${HOST_SERVER_URL}/v1/os-assignments/3"

echo "Waiting for Orchestrator to detect and stage the update..."

# Wait for the staging and countdown initiation
STAGED=false
for i in $(seq 1 15); do
    if limactl shell "${VM_NAME}" -- sudo journalctl -u orchestrator.service --since "1 minute ago" | grep -q "Started countdown for deferred OS update"; then
        echo -e "${GREEN}✓ Orchestrator successfully staged the update and started the timer.${NC}"
        BOOTC_STATUS_JSON=$(limactl shell "${VM_NAME}" -- sudo bootc status --json)
        echo "${BOOTC_STATUS_JSON}" | jq .
        STAGED=true
        break
    fi
    sleep 2
done

if [ "$STAGED" = false ]; then
    echo -e "${RED}𐄂 FAILED: Orchestrator did not start the deferred timer.${NC}"
    exit 1
fi

# Wait for the 5-second timer to expire and trigger the reboot process
echo "Waiting for 5-second timer to expire and trigger apply..."
APPLIED=false
for i in $(seq 1 10); do
    if limactl shell "${VM_NAME}" -- sudo journalctl -u orchestrator.service --since "1 minute ago" | grep -q "Timer expired! Locking application updates"; then
        echo -e "${GREEN}✓ Timer successfully expired and locked application updates!${NC}"
        BOOTC_STATUS_JSON=$(limactl shell "${VM_NAME}" -- sudo bootc status --json)
        echo "${BOOTC_STATUS_JSON}" | jq .
        APPLIED=true
        break
    fi
    sleep 1
done

if [ "$APPLIED" = false ]; then
    echo -e "${RED}𐄂 FAILED: Timer did not expire or fail to trigger the final upgrade.${NC}"
    exit 1
fi

echo -e "${GREEN}Deferred Bootc Upgrade E2E test passed successfully!${NC}"