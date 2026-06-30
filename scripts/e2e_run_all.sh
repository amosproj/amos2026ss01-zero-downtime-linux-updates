#!/usr/bin/env bash
# Test runner with background server lifecycle management

set -uo pipefail

# --- CONFIGURATION & STATE ---
readonly script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${script_dir}/tests/common_env.sh"

SERVER_PID=""
FAILED_COUNT=0
PASSED_COUNT=0
TPM_DIR="/tmp/emulated_tpm"

# TimescaleDB configurations
readonly timescale_container="amos-test-timescaledb"
readonly timescale_port=55433
readonly timescale_url="postgres://app:4M0S@127.0.0.1:${timescale_port}/amos_timeseries"
readonly devcontainer_dir="$(cd "$script_dir/../.devcontainer" && pwd)"

# Test execution sequence
TEST_SUITE=(
    "tests/e2e_seed_api.sh"
    "tests/e2e_tpm_init.sh"
    "tests/e2e_bootc_status.sh"
    "tests/e2e_bootc_upgrade.sh"
)

# This function executes immediately when the script finishes or hits an early abort
cleanup() {
    echo -e "\n${NC}=== Cleaning up background processes ==="
    
    limactl shell "${VM_NAME}" -- sudo systemctl stop orchestrator.service 2>/dev/null || true

    # Shut down the VM, also automatically terminates the backgrounded swtpm process
    echo "Stopping Lima VM '${VM_NAME}'..."
    limactl stop "${VM_NAME}" 2>/dev/null || true

    # 2. Terminate the mock server process group on the host machine
    if [ -n "${SERVER_PID:-}" ]; then
        echo "Stopping api-mock-server on host (PGID -${SERVER_PID})..."
        kill -- "-${SERVER_PID}" 2>/dev/null || true
        wait "${SERVER_PID}" 2>/dev/null || true
    fi

    # Terminate and clean up the TimescaleDB podman container
    echo "Stopping TimescaleDB container..."
    podman rm -f "$timescale_container" >/dev/null 2>&1 || true

    echo "========================================="
    echo "             FINAL SUMMARY               "
    echo "========================================="
    echo -e " Total Passed: ${GREEN}${PASSED_COUNT}${NC}"
    echo -e " Total Failed: ${RED}${FAILED_COUNT}${NC}"
    echo "========================================="

    if [ "${FAILED_COUNT}" -gt 0 ]; then
        echo -e "${RED}=== E2E TEST HARNESS FAILED ===${NC}"
        exit 1
    else
        echo -e "${GREEN}=== E2E TEST HARNESS PASSED ===${NC}"
        exit 0
    fi
}
trap cleanup EXIT

echo "========================================="
echo " Starting TPM and VM "
echo "========================================="

echo "Initializing emulated TPM in ${TPM_DIR}..."
pkill -f "swtpm.*${TPM_DIR}" 2>/dev/null || true
mkdir -p "${TPM_DIR}"
rm -f "${TPM_DIR}/swtpm-sock"
swtpm socket --tpm2 -d --tpmstate dir="${TPM_DIR}" --ctrl type=unixio,path="${TPM_DIR}/swtpm-sock" --log level=20

sleep 2

echo "Booting VM '${VM_NAME}' with QEMU TPM arguments..."
QEMU_SYSTEM_X86_64="qemu-system-x86_64 \
    -chardev socket,id=chrtpm,path=${TPM_DIR}/swtpm-sock \
    -tpmdev emulator,id=tpm0,chardev=chrtpm \
    -device tpm-tis,tpmdev=tpm0 \
    -smbios type=1,uuid=${DEVICE_UUID},serial=${DEVICE_SERIAL}" \
    limactl start "${VM_NAME}"

sleep 5

echo "Verifying SMBIOS UUID was applied inside the VM..."
ACTUAL_UUID=$(limactl shell "${VM_NAME}" -- sudo cat /sys/class/dmi/id/product_uuid | tr -d '[:space:]')
if [ "${ACTUAL_UUID}" != "${DEVICE_UUID}" ]; then
    echo -e "${RED}ERROR: SMBIOS UUID mismatch!${NC}" >&2
    echo -e "${RED}  Expected: ${DEVICE_UUID}${NC}" >&2
    echo -e "${RED}  Got:      ${ACTUAL_UUID}${NC}" >&2
    echo -e "${RED}Lima may not be forwarding QEMU_SYSTEM_X86_64. Check your Lima config.${NC}" >&2
    exit 1
fi
echo -e "${GREEN}SMBIOS UUID verified: ${ACTUAL_UUID}${NC}"

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
    if podman exec "$timescale_container" pg_isready -U postgres >/dev/null 2>&1; then
        sleep 1
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

echo "Clearing stale server instances..."
pkill -f amos-api-mock-server || true
sleep 0.5

APP_DATABASE_URL="sqlite::memory:" APP_TIMESCALE_DATABASE_URL="$timescale_url" setsid ../target/debug/amos-api-mock-server -dd &
SERVER_PID=$!

echo "Waiting for mock server to bind to port ${PORT}..."
MAX_ATTEMPTS=30
for i in $(seq 1 ${MAX_ATTEMPTS}); do
    if curl -s -o /dev/null "http://127.0.0.1:${PORT}/v1/tenants"; then
        echo "Mock server is up and listening."
        break
    fi
    
    if [ "$i" -eq ${MAX_ATTEMPTS} ]; then
        echo "Error: Server did not become ready within timeout period." >&2
        exit 1
    fi
    sleep 1
done

echo "========================================="
echo " Ensuring VM Pre-requisites "
echo "========================================="
# Ensure Podman API socket is active for application state tracking
limactl shell "${VM_NAME}" -- sudo systemctl enable --now podman.socket

echo "========================================="
echo " Running E2E Tests "
echo "========================================="

for test_script in "${TEST_SUITE[@]}"; do
    echo -e "\n---> Executing Phase: ${test_script}"
    
    if [ ! -x "$test_script" ]; then
        chmod +x "$test_script"
    fi

    if ./"$test_script"; then
        echo -e "${GREEN}✓ PASSED: ${test_script}${NC}"
        ((PASSED_COUNT++))
    else
        echo -e "${RED}𐄂 FAILED: ${test_script}${NC}"
        ((FAILED_COUNT++))
        
        # If seeding fails, stop immediately instead of cascading errors
        if [[ "$test_script" == *"seed_api"* ]]; then
            echo -e "${RED}Critical initialization failure in database seeding. Aborting matrix.${NC}"
            break
        fi
    fi
done

exit 0