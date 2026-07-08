#!/usr/bin/env bash
# Shared variables and helper utilities

# Do NOT set -e here, so we don't cause side effects when sourcing this script!
set -uo pipefail

# --- CONFIGURATION ---
export PORT=8080
export HOST_SERVER_URL="http://127.0.0.1:${PORT}"
export VM_NAME="edge-ipc"
export TPM_DIR="/tmp/emulated_tpm"

export DEVICE_UUID="00000000-0000-0000-0000-000000000001"
export DEVICE_SERIAL="AMOS-TEST-001"

# Color formatting for readable logs
export GREEN='\033[0;32m'
export RED='\033[0;31m'
export NC='\033[0m' # No Color

# Full RS512 JWT signed with the default dev key, valid 1000 years
export JWT='eyJhbGciOiJSUzUxMiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6Ik1hcmMgV2ViYmVyIiwiZXhwIjozMzMzNzg2MDc1Nn0.YLzANsYJj5TmCAURvMyUQSSeGk6fa8xrJhrbSrm999hMVxeYqTtT2c62dT7Ast9bdHENHWAPZD7OYWsOK2sCX-jqYfNFgmAmYxCtLaXMCVgIvqzOWf9miV8F5Zd8OaSnoaWbA7iXsICJ_kBYCP6zFdRQUoO-Evok4vtzH6Y5M1LyJtsy65NIpkpQt6DAZqf0s7818mrJdqpLp_L_1vqPq9QOrMen28lv_RNjWl5x9_lGhfw15TbGhfrE5mvmzsq6RW6M5Eun3CVGWXERqNzOqdVHo13BtmyRxLbJa8kP0r0qPubMfQf-bpAIVxG6oA5xbjytiEKQ8vfl1up6XBn429N_039-exEfv8EdZ35AjqLpLaSA4BM0RFurqZMse4ELJmNRPQLVMfrBDTf0yLB3USi0su3tFZRXQ6ND7cLpqL6PUYL0KrJZUiMwD8ZMSDBO7Rilh2thkhYp0EfBncIi5lI1gVlN5qSC51NJeDBRFPYnhH_-gwxecn1WzVILpiNki0E8euOpSTXgS2FNxlHhPfBevPodoBn8j-Vu0U9-8xmfqxZirGankWz4d00rthBn_B0IFKk0WFy742TW_Qs9NdAL9UnGJGwqYv88MtGo6vgfTwdE9WASkq4ubJ8GCvFmooKb9FrMGz_-9pS2RWRgO_kT_1PSD4bTMHQIMhC1eXs'

# Performs authenticated HTTP requests from the host and validates response codes.
api() {
    local path="$1"
    local method="$2"
    local body="$3"
    local expected_code="$4"

    local url="${HOST_SERVER_URL}${path}"
    local response actual_code

    if [ -n "$body" ]; then
        response=$(curl -sS -D - -X "$method" "$url" \
            -H "Content-Type: application/json" \
            -H "Authorization: Bearer ${JWT}" \
            -d "$body" \
            -w "\n%{http_code}")
    else
        response=$(curl -sS -D - -X "$method" "$url" \
            -H "Content-Type: application/json" \
            -H "Authorization: Bearer ${JWT}" \
            -w "\n%{http_code}")
    fi

    actual_code=$(printf '%s\n' "$response" | tail -n1)

    if [ "$actual_code" != "$expected_code" ]; then
        echo -e "${RED}Database Seeding Error [$method $path]: expected $expected_code, got $actual_code${NC}"
        test_failed=1
        exit 1
    fi
}

start_swtpm() {
    swtpm socket --tpm2 -t -d \
        --tpmstate dir="${TPM_DIR}" \
        --ctrl type=unixio,path="${TPM_DIR}/swtpm-sock" \
        --log level=20
}

stop_vm() {
    echo "Stopping Lima VM '${VM_NAME}'..."
    limactl stop -f "${VM_NAME}" 2>/dev/null || true
}
