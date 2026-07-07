#!/usr/bin/env bash
# Live-demo harness. Brings up the same full stack as e2e_run_all.sh -- mock
# cloud API + TimescaleDB on the host, plus the edge VM running the
# orchestrator -- primes it (registers + seeds the device) so it's ready to
# act on commands, then LEAVES EVERYTHING RUNNING instead of running tests and
# tearing down.
#
# The mock server's request log streams in this terminal so you can watch the
# API traffic. From another terminal you can:
#   * send custom API commands with scripts/demo_api.sh
#   * follow the orchestrator: limactl shell edge-ipc -- journalctl -u orchestrator.service -f
#
# Press Ctrl+C to tear the whole stack (VM, server, DB, TPM) down.
#
# The bring-up below deliberately mirrors e2e_run_all.sh; keep them in sync.

set -uo pipefail

source ./tests/common_env.sh

SERVER_PID=""
TPM_DIR="/tmp/emulated_tpm"

readonly timescale_container="amos-demo-timescaledb"
readonly timescale_port=55433
readonly timescale_url="postgres://app:4M0S@127.0.0.1:${timescale_port}/amos_timeseries"
readonly script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly devcontainer_dir="$(cd "$script_dir/../.devcontainer" && pwd)"

cleanup() {
    echo -e "\n${NC}=== Tearing down demo environment ==="

    limactl shell "${VM_NAME}" -- sudo systemctl stop orchestrator.service 2>/dev/null || true

    echo "Stopping and deleting Lima VM '${VM_NAME}'..."
    limactl stop "${VM_NAME}" -f 2>/dev/null || true
    limactl delete "${VM_NAME}" -f 2>/dev/null || true

    if [ -n "${SERVER_PID:-}" ]; then
        echo "Stopping api-mock-server (PGID -${SERVER_PID})..."
        kill -- "-${SERVER_PID}" 2>/dev/null || true
        wait "${SERVER_PID}" 2>/dev/null || true
    fi

    echo "Stopping TimescaleDB container..."
    podman rm -f "$timescale_container" >/dev/null 2>&1 || true

    echo "Demo environment stopped."
}
trap cleanup EXIT

echo "========================================="
echo " Starting TPM and VM "
echo "========================================="

echo "Cleaning up any existing TPM state in ${TPM_DIR}..."
rm -rf "${TPM_DIR}"

echo "Initializing emulated TPM in ${TPM_DIR}..."
if ! ./create_tpm.sh "$TPM_DIR"; then
    echo "Could not create TPM. Aborting."
    exit 1
fi
swtpm socket --tpm2 -d --tpmstate dir="${TPM_DIR}" --ctrl type=unixio,path="${TPM_DIR}/swtpm-sock" --log level=20

sleep 2

echo "Booting VM '${VM_NAME}' with QEMU TPM arguments..."
QEMU_SYSTEM_X86_64="qemu-system-x86_64 \
    -chardev socket,id=chrtpm,path=${TPM_DIR}/swtpm-sock \
    -tpmdev emulator,id=tpm0,chardev=chrtpm \
    -device tpm-tis,tpmdev=tpm0 \
    -smbios type=1,uuid=${DEVICE_UUID},serial=${DEVICE_SERIAL} \
    -smbios type=2,serial=${DEVICE_SERIAL}" \
    limactl start "${VM_NAME}"

sleep 5

echo "========================================="
echo " Starting TimescaleDB Container "
echo "========================================="

podman rm -f "$timescale_container" >/dev/null 2>&1 || true
echo "Starting throwaway TimescaleDB container..."
podman run -d --name "$timescale_container" \
    -e POSTGRES_PASSWORD=dummy \
    -p "127.0.0.1:${timescale_port}:5432" \
    -v "$devcontainer_dir/20_setup_timescale_db.sh:/docker-entrypoint-initdb.d/20_setup_timescale_db.sh:ro,Z" \
    docker.io/timescale/timescaledb:latest-pg18 >/dev/null

echo "Waiting for TimescaleDB to be completely ready..."
for i in $(seq 1 60); do
    if podman exec -e PGPASSWORD=4M0S "$timescale_container" \
            psql -U app -d amos_timeseries -c "SELECT 1" >/dev/null 2>&1; then
        echo "TimescaleDB is fully initialized and ready."
        break
    fi
    if [ "$i" -eq 60 ]; then
        echo "TimescaleDB did not become ready in time." >&2
        exit 1
    fi
    sleep 1
done

echo "========================================="
echo " Starting api-mock-server in Background "
echo "========================================="

echo "Building api-mock-server..."
cargo build --package amos-api-mock-server

echo "Clearing stale server instances..."
pkill -f amos-api-mock-server || true
sleep 0.5

APP_DATABASE_URL="sqlite::memory:" APP_TIMESCALE_DATABASE_URL="$timescale_url" setsid ./../target/debug/amos-api-mock-server -dd &
SERVER_PID=$!

echo "Waiting for mock server to bind to port ${PORT}..."
for i in $(seq 1 30); do
    if curl -s -o /dev/null "http://127.0.0.1:${PORT}/v1/tenants"; then
        echo "Mock server is up and listening."
        break
    fi
    if [ "$i" -eq 30 ]; then
        echo "Error: Server did not become ready within timeout period." >&2
        exit 1
    fi
    sleep 1
done

echo "========================================="
echo " Ensuring VM Pre-requisites "
echo "========================================="
limactl shell "${VM_NAME}" -- sudo systemctl is-active podman.socket
echo "Podman socket is running"

echo "========================================="
echo " Deploying Local Orchestrator Build "
echo "========================================="
DEV_VM="${VM_NAME}" make -C "$script_dir/.." dev-deploy

echo "========================================="
echo " Priming the Device (register + seed) "
echo "========================================="
# Reuse the e2e setup steps: register the device for onboarding (so the
# orchestrator self-registers) and seed the baseline OS version, so the demo
# starts from a device that is already known and ready to receive commands.
if ./tests/e2e_register_pending_device.sh && ./tests/e2e_seed_api.sh; then
    echo "Device registered and seeded."
else
    echo -e "${RED}WARNING: priming failed. The stack is up, but the device may not be" \
        "fully registered/seeded -- check the output above before demoing.${NC}" >&2
fi

cat <<BANNER

=========================================================
               DEMO ENVIRONMENT READY
=========================================================
  Mock cloud API : ${HOST_SERVER_URL}/v1   (request log streams below)
  Edge VM        : ${VM_NAME}   (device_id=1, uuid=${DEVICE_UUID})

  Send API commands (from another terminal, repo root):

    scripts/demo_api.sh GET  /v1/devices
    scripts/demo_api.sh GET  /v1/devices/1/summary

    # Deploy the hello-world app to the device, then watch it land:
    scripts/demo_api.sh POST /v1/applications \\
      '{ "name": "hello-world", "description": "demo app" }'
    scripts/demo_api.sh POST /v1/app-configs \\
      '{ "device_id": 1, "application_id": 1, "image": "ghcr.io/amosproj/amos2026ss01-zero-downtime-linux-updates-hello-world:latest", "config": { "environment": { "NAME": "AMOS" } } }'
    scripts/demo_api.sh POST /v1/app-assignments \\
      '{ "application_config_id": 1, "device_id": 1 }'
    scripts/demo_api.sh GET  /v1/reported-app-assignments?device_id=1

  Watch the orchestrator act on your commands:

    limactl shell ${VM_NAME} -- journalctl -u orchestrator.service -f

  Press Ctrl+C here to tear the whole stack down.
=========================================================

BANNER

# Block on the mock server so its request log keeps streaming here; Ctrl+C
# triggers the cleanup trap above.
wait "${SERVER_PID}"
