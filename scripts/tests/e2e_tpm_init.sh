#!/usr/bin/env bash
# Initializes the vTPM and registers the Edge IPC public key via the API

set -euo pipefail

cd "$(dirname "$0")/.."
source ./tests/common_env.sh

echo "=== Initializing vTPM on Edge IPC ==="

# Check if a persistent key already exists at 0x81000000
if ! limactl shell "${VM_NAME}" -- sudo tpm2_getcap handles-persistent | grep -q "0x81000000"; then
    echo "Generating new TPM primary and RSA keys..."

    # We execute this inside /tmp. Lima mounts the host working directory as read-only by default.
    # We must move to a writable directory on the VM to save the .ctx and .pub temporary files.
    limactl shell "${VM_NAME}" -- sudo bash -c '
        cd /tmp
        # Forcefully evict the handle first if it happens to be corrupted/mismatched
        tpm2_evictcontrol -C o -c 0x81000000 2>/dev/null || true

        tpm2_createprimary -C o -c primary.ctx
        tpm2_create -C primary.ctx -G rsa -u key.pub -r key.priv
        tpm2_load -C primary.ctx -u key.pub -r key.priv -c key.ctx
        tpm2_evictcontrol -C o -c key.ctx 0x81000000

        rm -f primary.ctx key.pub key.priv key.ctx
    '
else
    echo "TPM key already exists at handle 0x81000000."
fi

echo "Extracting public key from TPM..."
limactl shell "${VM_NAME}" -- sudo tpm2_readpublic -c 0x81000000 -f pem -o /tmp/pubkey.pem

# Read the PEM file and replace newlines with literal '\n' for the JSON payload
PUBKEY=$(limactl shell "${VM_NAME}" -- sudo sed -z 's/\n/\\n/g' /tmp/pubkey.pem)

echo "Registering public key with the API..."
# Note: We PUT to /v1/devices/1 assuming the seed script created this device ID
api "/v1/devices/1" PUT "{\"uuid\": \"${DEVICE_UUID}\", \"serial_number\": \"${DEVICE_SERIAL}\", \"tenant_id\": 1, \"public_key\": \"${PUBKEY}\"}" 200

echo -e "${GREEN}vTPM successfully initialized and public key registered.${NC}"

# Restart the orchestrator so it picks up the newly created TPM hardware keys and the new verbosity logging level
echo "Restarting Orchestrator to apply TPM keys and verbosity..."
limactl shell "${VM_NAME}" -- sudo systemctl restart orchestrator.service
