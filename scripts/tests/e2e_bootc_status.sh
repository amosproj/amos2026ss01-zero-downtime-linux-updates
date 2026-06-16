#!/usr/bin/env bash
# Validates bootc status & 'Up to Date' evaluation paths

cd "$(dirname "$0")"
source ./common_env.sh

echo "=== Testing Bootc Status Evaluation ==="

echo "Fetching active booted checksum from VM..."
BOOTED_HASH=$(limactl shell "${VM_NAME}" -- sudo bootc status --json | grep -o '"checksum":"[^"]*' | grep -o '[^"]*$' | head -n 1)

if [ -z "${BOOTED_HASH}" ]; then
    echo -e "${RED}Failed to extract live booted checksum from VM!${NC}"
    exit 1
fi
echo "Live VM is running deployment: ${BOOTED_HASH}"

# Tell the API that this matching hash is our target OS version
api "/v1/os-versions" POST "{\"commit_hash\": \"${BOOTED_HASH}\", \"orchestrator_version\": \"0.1.0\", \"description\": \"Current Base Baseline\"}" 201
api "/v1/os-assignments" POST '{"os_version_id": 2, "device_id": 1}' 201

# Trigger a polling loop check by restarting the agent service
echo "Restarting Orchestrator to trigger an active status evaluation..."
limactl shell "${VM_NAME}" -- sudo systemctl restart orchestrator.service

sleep 4

# Check the systemd log output to make sure it processed the match safely
echo "Checking logs to confirm Orchestrator verified 'Up to Date' criteria..."
if limactl shell "${VM_NAME}" -- sudo journalctl -u orchestrator.service -n 20 | grep -q "OS is up to date"; then
    echo -e "${GREEN}Success: Orchestrator safely verified status and matches targets.${NC}"
else
    echo -e "${RED}Failure: Orchestrator failed to log a safe 'Up to Date' status validation match.${NC}"
    exit 1
fi