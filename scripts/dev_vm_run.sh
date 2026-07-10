#!/usr/bin/env bash
# Boot a Lima VM running only the orchestrator -- no api-mock-server and no
# TimescaleDB -- and point it at an external cloud server. This is the "just a
# VM with the orchestrator" counterpart to e2e_run_all.sh: same TPM + QEMU boot,
# but nothing runs on the host and no tests execute. The VM is left running so
# you can interact with it; tear it down with `limactl delete -f <name>`.
#
# The VM must already have been `limactl create`d by the caller (the `dev-vm`
# Make target does this). Configuration is passed in via env:
#   VM_NAME       - name of the Lima VM to start
#   DEVICE_UUID   - SMBIOS product UUID injected via QEMU (the device's id)
#   DEVICE_SERIAL - SMBIOS product serial injected via QEMU
#   CLOUD_URL     - orchestrator cloud_url, e.g. http://192.168.1.10:8080/v1
#
# The orchestrator binary baked into the image is used as-is; no local build is
# deployed (that's what `make e2e` / `dev-deploy` are for).

set -euo pipefail

: "${VM_NAME:?VM_NAME must be set}"
: "${DEVICE_UUID:?DEVICE_UUID must be set}"
: "${DEVICE_SERIAL:?DEVICE_SERIAL must be set}"
: "${CLOUD_URL:?CLOUD_URL must be set}"

# Per-VM state dir to support several edge VMs, each with its own swtpm on one host
readonly TPM_DIR="${TPM_DIR:-/tmp/emulated_tpm/${VM_NAME}}"
readonly swtpm_pidfile="${TPM_DIR}/swtpm.pid"

GREEN='\033[0;32m'
NC='\033[0m'

echo "========================================="
echo " Starting TPM and VM "
echo "========================================="

# Kill any swtpm left running from a previous run, otherwise it keeps holding
# the TPM state dir/socket and the new swtpm below fails to start.
if [ -f "$swtpm_pidfile" ]; then
    kill "$(cat "$swtpm_pidfile")" 2>/dev/null || true
    rm -f "$swtpm_pidfile"
fi

echo "Cleaning up any existing TPM state in ${TPM_DIR}..."
rm -rf "${TPM_DIR}"

echo "Initializing emulated TPM in ${TPM_DIR}..."
if ! ./create_tpm.sh "$TPM_DIR"; then
    echo "Could not create TPM. Aborting." >&2
    exit 1
fi
swtpm socket --tpm2 -d --tpmstate dir="${TPM_DIR}" \
    --ctrl type=unixio,path="${TPM_DIR}/swtpm-sock" \
    --pid file="$swtpm_pidfile" --log level=20

sleep 2

echo "Booting VM '${VM_NAME}' with QEMU TPM arguments (device UUID ${DEVICE_UUID})..."
QEMU_SYSTEM_X86_64="qemu-system-x86_64 \
    -chardev socket,id=chrtpm,path=${TPM_DIR}/swtpm-sock \
    -tpmdev emulator,id=tpm0,chardev=chrtpm \
    -device tpm-tis,tpmdev=tpm0 \
    -smbios type=1,uuid=${DEVICE_UUID},serial=${DEVICE_SERIAL} \
    -smbios type=2,serial=${DEVICE_SERIAL}" \
    limactl start "${VM_NAME}"

sleep 5

echo "========================================="
echo " Ensuring VM Pre-requisites "
echo "========================================="
# The orchestrator talks to podman over its API socket.
limactl shell "${VM_NAME}" -- sudo systemctl is-active podman.socket
echo "Podman socket is running"

echo "========================================="
echo " Pointing orchestrator at ${CLOUD_URL} "
echo "========================================="
# Overwrite the config the Lima template baked in (which points at the
# host-local mock server) so the orchestrator polls the external server instead.
limactl shell "${VM_NAME}" -- sudo mkdir -p /etc/amos
limactl shell "${VM_NAME}" -- sudo tee /etc/amos/config.toml >/dev/null <<EOF
cloud_url = "${CLOUD_URL}"
poll_interval_secs = 5
EOF

# Restart the (image-baked) orchestrator so it picks up the config we just wrote.
limactl shell "${VM_NAME}" -- sudo systemctl restart orchestrator.service

echo
echo -e "${GREEN}=== VM '${VM_NAME}' is up ===${NC}"
echo "  Device UUID : ${DEVICE_UUID}"
echo "  Cloud URL   : ${CLOUD_URL}"
echo "  Tail logs   : limactl shell ${VM_NAME} -- journalctl -u orchestrator.service -f"
echo "  Tear down   : limactl stop ${VM_NAME} -f && limactl delete ${VM_NAME} -f"
