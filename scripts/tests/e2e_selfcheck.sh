#!/usr/bin/env bash
# Validates hello-world application deployment and reporting

set -euo pipefail

cd "$(dirname "$0")"
source ./common_env.sh

echo "=== Testing device self checks ==="

# Case: Everything should works as normal
echo "Testing for self check pass under normal operation..."
limactl shell "${VM_NAME}" -- bash -c 'sudo /var/usrlocal/bin/amos-orchestrator -s ; exit $?'

# Case: Broken/not running Podman socket should fail
echo -e "\nTesting for self check fail with broken/not running Podman socket..."
limactl shell "${VM_NAME}" -- sudo systemctl stop podman.socket
limactl shell "${VM_NAME}" -- bash -c 'sudo /var/usrlocal/bin/amos-orchestrator -s 2>&1 \
    | grep "Could not connect to Podman socket"'
stop_vm

# Case: Missing TPM should lead to failure
echo "Testing for self check fail on missing TPM..."
QEMU_SYSTEM_X86_64="qemu-system-x86_64 \
    -smbios type=1,uuid=${DEVICE_UUID},serial=${DEVICE_SERIAL} \
    -smbios type=2,serial=${DEVICE_SERIAL}" \
    limactl start --log-level warn "${VM_NAME}"
limactl shell "${VM_NAME}" -- bash -c 'sudo /var/usrlocal/bin/amos-orchestrator -s 2>&1 \
    | grep "Could not initialize the TPM"'
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
    limactl start --log-level warn "${VM_NAME}"
limactl shell "${VM_NAME}" -- bash -c 'sudo /var/usrlocal/bin/amos-orchestrator -s 2>&1 \
    | grep "Could not read read endorsement key"'
stop_vm

# Case: Failing to read DMI info should fail
echo -e "\nTesting for self check fail on unavailable UUID info..."
start_swtpm
QEMU_SYSTEM_X86_64="qemu-system-x86_64 \
    -chardev socket,id=chrtpm,path=${TPM_DIR}/swtpm-sock \
    -tpmdev emulator,id=tpm0,chardev=chrtpm \
    -device tpm-tis,tpmdev=tpm0 \
    -smbios type=2,serial=${DEVICE_SERIAL}" \
    limactl start --log-level warn "${VM_NAME}"
limactl shell "${VM_NAME}" -- bash -c 'sudo /var/usrlocal/bin/amos-orchestrator -s 2>&1 \
    | grep "Could not read device UUID"'
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
    limactl start --log-level warn "${VM_NAME}"
sleep 5
