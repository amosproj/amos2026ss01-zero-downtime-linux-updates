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
    # Kill any swtpm from a previous case still holding this socket: the new
    # instance can't bind it, and the stale one keeps answering the next
    # qemu's TPM chardev, delaying boot past lima's QMP-dial timeout.
    pkill -f "swtpm socket.*${TPM_DIR}/swtpm-sock" 2>/dev/null || true

    swtpm socket --tpm2 -t -d \
        --tpmstate dir="${TPM_DIR}" \
        --ctrl type=unixio,path="${TPM_DIR}/swtpm-sock" \
        --log level=20
}

stop_vm() {
    echo "Stopping Lima VM '${VM_NAME}'..."
    limactl stop -f "${VM_NAME}" 2>/dev/null || true
}

# Start the VM, tolerating lima's "FATA degraded" exit.
#
# `limactl start` exits non-zero if the guest agent doesn't stabilize, which is
# only about port forwarding and file sharing. `limactl shell` is plain ssh and
# keeps working, so for these tests a degraded VM is still perfectly usable.
# Wait for ssh instead of trusting the exit code.
start_vm_allow_degraded() {
    limactl start --log-level warn "${VM_NAME}" || true

    for _ in $(seq 60); do
        if limactl shell "${VM_NAME}" -- true 2>/dev/null; then
            return 0
        fi
        sleep 5
    done

    echo -e "${RED}VM '${VM_NAME}' never became reachable over ssh${NC}" >&2
    return 1
}

# Stop greenboot from rebooting the VM when its health check fails.
#
# Fedora ships greenboot-rs (the Rust rewrite), which collapsed the old
# per-stage units into a single greenboot-healthcheck.service and calls
# `systemctl reboot` from inside it. There is no separate reboot unit to mask,
# so the only way to stop the reboot is to stop the check from running.
#
# Setting GREENBOOT_MAX_BOOT_ATTEMPTS=0 is not enough: greenboot-rs only reads
# that config when boot_counter is unset. If a previous crashed run left a
# counter behind, it takes the `Some(counter) if counter > 0` branch and
# reboots regardless. So: mask the unit, and clear the grub state too.
#
# The tradeoff is that the health check no longer runs on the fault-injection
# boots, so those boots can't assert on greenboot's verdict -- the tests assert
# on `amos-orchestrator -s` directly instead.
disable_greenboot_reboot() {
    limactl shell "${VM_NAME}" -- sudo systemctl mask greenboot-healthcheck.service
    reset_greenboot_grub_state
}

enable_greenboot_reboot() {
    limactl shell "${VM_NAME}" -- sudo systemctl unmask greenboot-healthcheck.service
}

# EXIT-trap version of the above: re-arm greenboot on the way out, whether the
# test passed or bailed out mid-way, so the bootc rollback tests that run after
# us don't inherit a VM with its health check masked. Must not mask the script's
# own exit status, and must survive being called with the VM stopped or in
# whatever state a failed assertion left it.
restore_greenboot_reboot() {
    local status=$?

    # `|| true`: under `set -e` a nonzero limactl (e.g. no such instance) would
    # abort the trap and overwrite the script's real exit status with its own.
    local vm_status
    vm_status=$(limactl list --format '{{.Status}}' "${VM_NAME}" 2>/dev/null) || true
    if [ "$vm_status" = "Running" ]; then
        enable_greenboot_reboot || true
        # Leave grub clean, so the next test's first boot doesn't inherit a
        # counter that sends greenboot straight into a reboot or a rollback.
        reset_greenboot_grub_state || true
    else
        # The mask and the grub vars live on the VM's disk, so they outlive this
        # script. We can't undo them without a running VM; say so rather than
        # leaving a later test to fail for reasons that look unrelated.
        echo -e "${RED}Warning: VM '${VM_NAME}' is not running (status: ${vm_status:-unknown});" \
            "greenboot-healthcheck.service is left masked. Recreate the VM before running" \
            "tests that rely on greenboot rollback.${NC}" >&2
    fi

    return "$status"
}

# `limactl start` returns as soon as ssh answers, which can be before
# greenboot-healthcheck.service has finished running the checks. Block until it
# has settled into a terminal state, so the assertions below don't read a verdict
# that doesn't exist yet.
wait_for_greenboot() {
    local state

    # Wait for boot to complete first. greenboot-healthcheck is a oneshot that
    # reads "inactive" both before it starts and (on some paths) after it ends,
    # so polling the unit alone can mistake "not started yet" for "done".
    for _ in $(seq 60); do
        state=$(limactl shell "${VM_NAME}" -- systemctl is-system-running 2>/dev/null) || true
        case "$state" in
            initializing | starting | "") sleep 5 ;;
            *) break ;;
        esac
    done

    # RemainAfterExit=yes keeps a passed check "active" and a failed one
    # "failed"; anything else means it is still running.
    for _ in $(seq 60); do
        state=$(limactl shell "${VM_NAME}" -- systemctl is-active greenboot-healthcheck.service 2>/dev/null) || true
        case "$state" in
            active | failed) return 0 ;;
        esac
        sleep 5
    done

    echo -e "${RED}greenboot-healthcheck.service did not settle (last state: ${state:-unknown})${NC}" >&2
    return 1
}

# The health check ran at boot and passed. There is no assert_boot_red
# counterpart: the fault-injection boots run with greenboot-healthcheck.service
# masked (see disable_greenboot_reboot), so there is no verdict to assert on.
assert_boot_green() {
    wait_for_greenboot

    if ! limactl shell "${VM_NAME}" -- systemctl is-active --quiet greenboot-healthcheck.service; then
        echo -e "${RED}Expected greenboot-healthcheck.service to have succeeded, but it did not${NC}" >&2
        return 1
    fi

    if ! limactl shell "${VM_NAME}" -- sudo grub2-editenv - list | grep -qx 'boot_success=1'; then
        echo -e "${RED}Expected grub to record a green boot (boot_success=1)${NC}" >&2
        return 1
    fi
}

# Clear the red boot that a failing health check left behind. Without this, a
# leftover boot_counter makes greenboot reboot on the next failing boot before
# it ever looks at its config, and a leftover greenboot_rollback_trigger sends
# it into a rollback once that counter hits zero.
# /boot is mounted read-only on bootc systems, hence the remount dance.
reset_greenboot_grub_state() {
    limactl shell "${VM_NAME}" -- sudo mount -o remount,rw /boot
    limactl shell "${VM_NAME}" -- sudo grub2-editenv - set boot_success=1
    limactl shell "${VM_NAME}" -- sudo grub2-editenv - unset boot_counter
    limactl shell "${VM_NAME}" -- sudo grub2-editenv - unset greenboot_rollback_trigger
    limactl shell "${VM_NAME}" -- sudo mount -o remount,ro /boot
}

# Starts the VM non-interactively if it isn't already running, so that
# `limactl shell` never hits its "Do you want to start the instance now?" prompt.
ensure_vm_running() {
    local status
    status=$(limactl list --format '{{.Status}}' "${VM_NAME}" 2>/dev/null)
    if [ "$status" != "Running" ]; then
        echo "VM '${VM_NAME}' is not running (status: ${status:-unknown}); starting it..."
        start_swtpm
        QEMU_SYSTEM_X86_64="qemu-system-x86_64 \
            -chardev socket,id=chrtpm,path=${TPM_DIR}/swtpm-sock \
            -tpmdev emulator,id=tpm0,chardev=chrtpm \
            -device tpm-tis,tpmdev=tpm0 \
            -smbios type=1,uuid=${DEVICE_UUID},serial=${DEVICE_SERIAL} \
            -smbios type=2,serial=${DEVICE_SERIAL}" \
            limactl start --log-level warn "${VM_NAME}"
        sleep 5
    fi
}
