#!/usr/bin/env bash
# Test runner with background server lifecycle management

set -uo pipefail

# --- CONFIGURATION & STATE ---
export PORT=8080
export VM_NAME="edge-ipc"
SERVER_PID=""
FAILED_COUNT=0
PASSED_COUNT=0
TPM_DIR="/tmp/emulated_tpm"

# Color outputs
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m'

# Test execution sequence
TEST_SUITE=(
    "tests/e2e_seed_api.sh"
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
mkdir -p "${TPM_DIR}"
swtpm socket --tpm2 -d --tpmstate dir="${TPM_DIR}" --ctrl type=unixio,path="${TPM_DIR}/swtpm-sock" --log level=20

sleep 5

echo "Booting VM '${VM_NAME}' with QEMU TPM arguments..."
set -x
QEMU_SYSTEM_X86_64="qemu-system-x86_64 \
    -chardev socket,id=chrtpm,path=${TPM_DIR}/swtpm-sock \
    -tpmdev emulator,id=tpm0,chardev=chrtpm \
    -device tpm-tis,tpmdev=tpm0" \
    limactl --debug start "${VM_NAME}"
set +x

sleep 2

echo "========================================="
echo " Starting api-mock-server in Background "
echo "========================================="

echo "Clearing stale server instances..."
pkill -f amos-api-mock-server || true
sleep 0.5

APP_DATABASE_URL="sqlite::memory:" setsid ./../target/debug/amos-api-mock-server -dd &
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
    sleep 0.5
done

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