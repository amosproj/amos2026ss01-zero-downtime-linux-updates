#!/usr/bin/env bash
# Verifies that an unsigned OS image is explicitly rejected and system is not updated

cd "$(dirname "$0")"
source ./common_env.sh

echo "=== Testing Unsigned OS Image Denial ==="

# Clear out all old assignments to avoid processing stale targets (like switch-2-tag)
echo "Cleaning up prior assignments from database..."
for id in $(seq 1 4); do
    curl -s -X DELETE -H "Authorization: Bearer ${JWT}" "${HOST_SERVER_URL}/v1/os-assignments/${id}" > /dev/null
done

# Define a target image reference that lacks a valid signature
UNSIGNED_IMAGE_REF="ghcr.io/amosproj/amos2026ss01-zero-downtime-linux-updates-system:commit-d8a6120"

# Record the exact baseline checksum before forcing an update attempt
BEFORE_CHECKSUM=$(limactl shell "${VM_NAME}" -- sudo bootc status --json | jq -r '.status.booted.ostree.checksum')

# Seed the unsigned image target
echo "Seeding assignment for unsigned image: ${UNSIGNED_IMAGE_REF}"
api "/v1/os-versions" POST "{\"commit_hash\": \"${UNSIGNED_IMAGE_REF}\", \"orchestrator_version\": \"0.1.0\", \"description\": \"Unsigned Test Image\"}" 201
api "/v1/os-assignments" POST '{"os_version_id": 5, "device_id": 1, "immediate": true}' 201

# Restart Orchestrator to force verification processing
echo "Restarting Orchestrator to trigger validation..."
limactl shell "${VM_NAME}" -- sudo systemctl restart orchestrator.service
sleep 6

# Verify bootc/OS-specific signature denial in logs
echo "Verifying log enforcement..."
# Look specifically for bootc output failures regarding signature verification
if limactl shell "${VM_NAME}" -- sudo journalctl -u orchestrator.service --since "10 seconds ago" | grep -i "bootc switch" -A 5 | grep -qE "signature|rejected|denied"; then
    echo -e "${GREEN}Log Check Passed: OS signature policy enforcement caught the violation.${NC}"
else
    echo -e "${RED}Log Check Failed: Could not isolate bootc signature rejection logs!${NC}"
    exit 1
fi

# Final State Assertion: The OS checksum MUST NOT have changed
AFTER_CHECKSUM=$(limactl shell "${VM_NAME}" -- sudo bootc status --json | jq -r '.status.booted.ostree.checksum')

if [ "${BEFORE_CHECKSUM}" = "${AFTER_CHECKSUM}" ]; then
    echo -e "${GREEN}State Check Passed: System safely blocked deployment of the unsigned image.${NC}"
else
    echo -e "${RED}Critical Failure: The system updated to an unsigned image!${NC}"
    exit 1
fi