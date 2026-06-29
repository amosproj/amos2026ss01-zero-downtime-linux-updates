#!/usr/bin/env bash
# Registers the device for automatic onboarding

set -euo pipefail

cd "$(dirname "$0")/.."
source ./tests/common_env.sh

echo "=== Setting up Edge IPC for automatic onboarding ==="

device_ensorsement_key=$(limactl shell "${VM_NAME}" -- \
  sudo tpm2_readpublic -c 0x81010001 -f pem -o /dev/stdout | openssl rsa -pubin 2>/dev/null | sed -z 's/\n/\\n/g'
)

payload=$(cat <<EOF
{
  "serial_number": "${DEVICE_SERIAL}",
  "endorsement_public_key": "${device_ensorsement_key}"
}
EOF
)

# We need to have a tenant here cause a newly registered device is assigned to the first known tenant (device MUST have a tenant)
api "/v1/tenants" POST '{ "name": "Weber-Lager", "description": "Automated Testing Tenant" }' 201
api "/v1/pending-device-registrations" POST "$payload" 201

echo -e "${GREEN}Device endorsement public key successfully registered for the pending device registration.${NC}"

# Restart the orchestrator so it tries to register itself immediately and wait for that output
echo "Restarting Orchestrator to make it register itself immediately..."

limactl shell "${VM_NAME}" -- sudo systemctl restart orchestrator.service
sleep 5

limactl shell "${VM_NAME}" -- journalctl -u orchestrator.service --since "6 seconds ago" | \
  grep "Successfully self-registered device"
