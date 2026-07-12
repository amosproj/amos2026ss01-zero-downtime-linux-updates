#!/usr/bin/env bash
# Validates hello-world application deployment and reporting

set -euo pipefail

cd "$(dirname "$0")"
source ./common_env.sh

echo "=== Testing device self checks ==="

# Case: Everything should works as normal
echo "Testing for self check pass under normal operation..."
limactl shell "${VM_NAME}" -- bash -c 'sudo /var/usrlocal/bin/amos-orchestrator -s ; exit $?'
assert_boot_green

# Case: Broken/not running Podman socket should fail
echo -e "\nTesting for self check fail with broken/not running Podman socket..."
limactl shell "${VM_NAME}" -- sudo systemctl stop podman.socket
limactl shell "${VM_NAME}" -- bash -c 'sudo /var/usrlocal/bin/amos-orchestrator -s 2>&1 \
    | grep "Could not connect to Podman socket"'

# Every case below boots a VM whose greenboot health check
# (/etc/greenboot/check/required.d/10-orchestrator-check.sh, i.e. the very
# `amos-orchestrator -s` we are asserting on) is guaranteed to fail. Greenboot
# reacts by rebooting once per remaining grub boot_counter attempt; lima sees
# the guest agent die with each reboot and aborts `limactl start` with
# "FATA degraded". Mask the health check and clear the grub state so those
# boots come up once and stay up. The EXIT trap undoes both.
disable_greenboot_reboot
trap restore_greenboot_reboot EXIT
stop_vm

# Case: Missing TPM should lead to failure
echo "Testing for self check fail on missing TPM..."
QEMU_SYSTEM_X86_64="qemu-system-x86_64 \
    -smbios type=1,uuid=${DEVICE_UUID},serial=${DEVICE_SERIAL} \
    -smbios type=2,serial=${DEVICE_SERIAL}" \
    start_vm_allow_degraded
limactl shell "${VM_NAME}" -- bash -c 'sudo /var/usrlocal/bin/amos-orchestrator -s 2>&1 \
    | grep "Could not initialize the TPM"'
reset_greenboot_grub_state
stop_vm

# Case: Wrongly/unexpectedly intialized TPM should fail
echo -e "\nTesting for self check fail on unexpectedly initialized TPM..."
broken_tpm_dir=`mktemp -d`
echo "Creating broken vTPM"
swtpm_setup --tpm2 --tpmstate "$broken_tpm_dir" --lock-nvram
swtpm socket --tpm2 -d --tpmstate dir="${broken_tpm_dir}" --ctrl type=unixio,path="${broken_tpm_dir}/swtpm-sock"
QEMU_SYSTEM_X86_64="qemu-system-x86_64 \
    -chardev socket,id=chrtpm,path=${broken_tpm_dir}/swtpm-sock \
    -tpmdev emulator,id=tpm0,chardev=chrtpm \
    -device tpm-tis,tpmdev=tpm0 \
    -smbios type=1,uuid=${DEVICE_UUID},serial=${DEVICE_SERIAL} \
    -smbios type=2,serial=${DEVICE_SERIAL}" \
    start_vm_allow_degraded
limactl shell "${VM_NAME}" -- bash -c 'sudo /var/usrlocal/bin/amos-orchestrator -s 2>&1 \
    | grep "Could not read read endorsement key"'
reset_greenboot_grub_state
stop_vm

# Case: Failing to read DMI info should fail
echo -e "\nTesting for self check fail on unavailable UUID info..."
start_swtpm
QEMU_SYSTEM_X86_64="qemu-system-x86_64 \
    -chardev socket,id=chrtpm,path=${TPM_DIR}/swtpm-sock \
    -tpmdev emulator,id=tpm0,chardev=chrtpm \
    -device tpm-tis,tpmdev=tpm0 \
    -smbios type=2,serial=${DEVICE_SERIAL}" \
    start_vm_allow_degraded
limactl shell "${VM_NAME}" -- bash -c 'sudo /var/usrlocal/bin/amos-orchestrator -s 2>&1 \
    | grep "Could not read device UUID"'
reset_greenboot_grub_state
stop_vm

# Restore "normal" vm
echo -e "\nRestoring normally running VM for further tests..."
start_swtpm
QEMU_SYSTEM_X86_64="qemu-system-x86_64 \
    -chardev socket,id=chrtpm,path=${TPM_DIR}/swtpm-sock \
    -tpmdev emulator,id=tpm0,chardev=chrtpm \
    -device tpm-tis,tpmdev=tpm0 \
    -smbios type=1,uuid=${DEVICE_UUID},serial=${DEVICE_SERIAL} \
    -smbios type=2,serial=${DEVICE_SERIAL}" \
    start_vm_allow_degraded
sleep 5

# No assert_boot_green here: this VM booted while greenboot-healthcheck.service
# was still masked, so it has no verdict to report. The EXIT trap unmasks it and
# clears the grub state for the bootc switch/rollback tests that run next.
