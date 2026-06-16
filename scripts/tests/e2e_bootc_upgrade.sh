#!/usr/bin/env bash
# Validates update loops via bootc switch and apply

cd "$(dirname "$0")"
source ./common_env.sh

echo "=== Testing Bootc Switch & Apply Sequence ==="

# Post an entirely new target image version to the mock API server
TARGET_UPGRADE_HASH="092599a804d5169ae2a0a306bcb4b213b7646d28"
echo "Registering new upgrade target: ${TARGET_UPGRADE_HASH}"

api "/v1/os-versions" POST "{\"commit_hash\": \"${TARGET_UPGRADE_HASH}\", \"orchestrator_version\": \"0.1.0\", \"description\": \"Target Upgrade Image Context\"}" 201
api "/v1/os-assignments" POST '{"os_version_id": 3, "device_id": 1}' 201

# Force orchestrator agent iteration check
echo "Restarting Orchestrator loop..."
limactl shell "${VM_NAME}" -- sudo systemctl restart orchestrator.service

echo "Awaiting Orchestrator upgrade trigger phase (allowing time for execution processing)..."
sleep 6

# Confirm that the application launched both its switch & staging actions
echo "Parsing deployment execution logs..."
LOG_OUTPUT=$(limactl shell "${VM_NAME}" -- sudo journalctl -u orchestrator.service -n 50)

if echo "${LOG_OUTPUT}" | grep -q "Switching OS image"; then
    echo -e "${GREEN}Success: Found 'bootc switch' execution command logs.${NC}"
else
    echo -e "${RED}Failure: Orchestrator never initiated an image switch loop operation.${NC}"
    exit 1
fi

if echo "${LOG_OUTPUT}" | grep -q "bootc switch staged successfully"; then
    echo -e "${GREEN}Success: 'bootc apply' command staging processing was logged successfully.${NC}"
else
    echo -e "${RED}Warning/Failure: The switch command failed or apply sequence didn't finish staging.${NC}"
    exit 1
fi