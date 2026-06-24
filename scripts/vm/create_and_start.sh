set -eu

# cd into the correct dir
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
target="$script_dir/../../"
pushd "$target" >/dev/null || exit 1

readonly server_dir="$target/api-mock-server"
readonly devcontainer_dir="$target/.devcontainer"
readonly api_base_path="http://localhost:8080/v1"

readonly timescale_container="amos-vm-timescaledb"
readonly timescale_port=55433
readonly timescale_url="postgres://app:4M0S@127.0.0.1:${timescale_port}/amos_timeseries"

# Same default dev JWT used by scripts/test_logs.sh, signed for the default
# dev key in api-mock-server/src/config.rs, valid 1000 years.
readonly jwt='eyJhbGciOiJSUzUxMiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6Ik1hcmMgV2ViYmVyIiwiZXhwIjozMzMzNzg2MDc1Nn0.YLzANsYJj5TmCAURvMyUQSSeGk6fa8xrJhrbSrm999hMVxeYqTtT2c62dT7Ast9bdHENHWAPZD7OYWsOK2sCX-jqYfNFgmAmYxCtLaXMCVgIvqzOWf9miV8F5Zd8OaSnoaWbA7iXsICJ_kBYCP6zFdRQUoO-Evok4vtzH6Y5M1LyJtsy65NIpkpQt6DAZqf0s7818mrJdqpLp_L_1vqPq9QOrMen28lv_RNjWl5x9_lGhfw15TbGhfrE5mvmzsq6RW6M5Eun3CVGWXERqNzOqdVHo13BtmyRxLbJa8kP0r0qPubMfQf-bpAIVxG6oA5xbjytiEKQ8vfl1up6XBn429N_039-exEfv8EdZ35AjqLpLaSA4BM0RFurqZMse4ELJmNRPQLVMfrBDTf0yLB3USi0su3tFZRXQ6ND7cLpqL6PUYL0KrJZUiMwD8ZMSDBO7Rilh2thkhYp0EfBncIi5lI1gVlN5qSC51NJeDBRFPYnhH_-gwxecn1WzVILpiNki0E8euOpSTXgS2FNxlHhPfBevPodoBn8j-Vu0U9-8xmfqxZirGankWz4d00rthBn_B0IFKk0WFy742TW_Qs9NdAL9UnGJGwqYv88MtGo6vgfTwdE9WASkq4ubJ8GCvFmooKb9FrMGz_-9pS2RWRgO_kT_1PSD4bTMHQIMhC1eXs'

readonly swtpm_dir=/tmp/emulated_tpm
readonly swtpm_pidfile="$swtpm_dir/swtpm.pid"

server_pid=

cleanup() {
    if [ -n "$server_pid" ]; then
        echo "Stopping api-mock-server (pid $server_pid)..."
        kill -- "-$server_pid" 2>/dev/null || true
        wait "$server_pid" 2>/dev/null || true
    fi

    echo "Stopping TimescaleDB container..."
    podman rm -f "$timescale_container" >/dev/null 2>&1 || true

    echo "Stopping and deleting VM..."
    limactl stop edge-ipc -f || true
    sleep 2
    limactl delete edge-ipc -f || true
    sleep 2

    if [ -f "$swtpm_pidfile" ]; then
        echo "Stopping swtpm..."
        kill "$(cat "$swtpm_pidfile")" 2>/dev/null || true
        rm -f "$swtpm_pidfile"
    fi

    popd >/dev/null
}
trap cleanup EXIT

# Remove vm if already exists
limactl stop edge-ipc -f || true
sleep 2
limactl delete edge-ipc -f || true
sleep 2

# Kill any swtpm left running from a previous (e.g. interrupted) run, since
# it would otherwise hold the TPM state dir/socket and make the new swtpm
# instance below fail to start, which in turn makes QEMU's vTPM device fail.
if [ -f "$swtpm_pidfile" ]; then
    kill "$(cat "$swtpm_pidfile")" 2>/dev/null || true
    rm -f "$swtpm_pidfile"
fi

# Create swtpm socket dir
mkdir -p "$swtpm_dir"

# Create swtpm socket
swtpm socket --tpm2 -d --tpmstate dir="$swtpm_dir" --ctrl type=unixio,path="$swtpm_dir/swtpm-sock" --pid file="$swtpm_pidfile" --log level=20

# Start the throwaway TimescaleDB container now, in parallel with VM
# creation/boot/TPM provisioning below, so it has time to become ready
# before we actually need it (right before starting api-mock-server).
podman rm -f "$timescale_container" >/dev/null 2>&1 || true
echo "Starting TimescaleDB container..."
podman run -d --name "$timescale_container" \
    -e POSTGRES_PASSWORD=dummy \
    -p "127.0.0.1:${timescale_port}:5432" \
    -v "$devcontainer_dir/20_setup_timescale_db.sh:/docker-entrypoint-initdb.d/20_setup_timescale_db.sh:ro,Z" \
    docker.io/timescale/timescaledb:latest-pg18 >/dev/null

# Create new Lima VM
limactl create -y --name edge-ipc dev-env/lima/edge-ipc.yaml

# Start VM with TPM support
QEMU_SYSTEM_X86_64="qemu-system-x86_64 \
                    -chardev socket,id=chrtpm,path=/tmp/emulated_tpm/swtpm-sock \
                    -tpmdev emulator,id=tpm0,chardev=chrtpm \
                    -device tpm-tis,tpmdev=tpm0" \
    limactl start edge-ipc

# Wait for the vTPM device to show up inside the VM
echo "Waiting for /dev/tpm0 inside the VM..."
until limactl shell edge-ipc -- test -e /dev/tpm0 2>/dev/null; do
    sleep 1
done

# Initialize the TPM and persist a signing key, unless one already exists
TPM_KEY_HANDLE=0x81000000
TPM_WORKDIR=/tmp/tpm-init

if limactl shell edge-ipc -- sudo tpm2_getcap handles-persistent | grep -q "$TPM_KEY_HANDLE"; then
    echo "TPM signing key already persisted at $TPM_KEY_HANDLE"
else
    echo "Initializing TPM and creating persistent signing key..."
    limactl shell edge-ipc -- sudo bash -c "
        set -eu
        trap 'echo \"TPM init failed at: \$BASH_COMMAND\" >&2' ERR
        mkdir -p '$TPM_WORKDIR'
        cd '$TPM_WORKDIR'
        tpm2_createprimary -C o -c primary.ctx
        tpm2_create -C primary.ctx -G rsa -u key.pub -r key.priv \
            -a 'sign|fixedtpm|fixedparent|sensitivedataorigin|userwithauth'
        tpm2_load -C primary.ctx -u key.pub -r key.priv -c key.ctx
        tpm2_evictcontrol -C o -c key.ctx '$TPM_KEY_HANDLE'
        tpm2_getcap handles-persistent
    " || { echo "TPM initialization failed" >&2; exit 1; }
fi

# Sanity-check the persisted key by signing and verifying a test file
echo "Testing TPM signing key..."
limactl shell edge-ipc -- sudo bash -c "
    set -eu
    trap 'echo \"TPM sign/verify test failed at: \$BASH_COMMAND\" >&2' ERR
    mkdir -p '$TPM_WORKDIR'
    cd '$TPM_WORKDIR'
    tpm2_readpublic -c '$TPM_KEY_HANDLE' -f pem -o pubkey.pem
    openssl rsa -pubin -in pubkey.pem -text -noout
    date > data.txt
    tpm2_sign -c '$TPM_KEY_HANDLE' -g sha256 -f plain -o sig.bin data.txt
    openssl dgst -sha256 -verify pubkey.pem -signature sig.bin data.txt
" || { echo "TPM sign/verify test failed" >&2; exit 1; }

# Copy the TPM public key out of the VM so the host can register it with the
# api-mock-server below.
limactl shell edge-ipc -- sudo cat "$TPM_WORKDIR/pubkey.pem" >/tmp/my_tpm_pubkey.pem

echo "Waiting for TimescaleDB to be ready..."
# Check the actual TCP endpoint api-mock-server connects to, not the socket
# inside the container: the postgres image's entrypoint runs initdb scripts
# against a temporary instance that only listens on a Unix socket, then
# restarts into the real (TCP-listening) server. Checking readiness via
# `podman exec ... pg_isready` (no host/port -> Unix socket) can report
# ready against that temporary instance, before the TCP listener used below
# is actually up, racing api-mock-server's connection against the restart.
for i in $(seq 1 60); do
    if pg_isready -h 127.0.0.1 -p "$timescale_port" -U postgres >/dev/null 2>&1; then
        echo "TimescaleDB is ready."
        break
    fi
    if [ "$i" -eq 60 ]; then
        echo "TimescaleDB did not become ready in time." >&2
        exit 1
    fi
    sleep 1
done

# Start the api-mock-server in its own process group (set -m) so that
# kill -- -$pid reaches both cargo and the server binary it spawns.
echo "Starting api-mock-server..."
set -m
APP_DATABASE_URL="sqlite::memory:" APP_TIMESCALE_DATABASE_URL="$timescale_url" \
    cargo run --manifest-path "$server_dir/Cargo.toml" -- -ddd &
server_pid=$!
set +m

echo "Waiting for api-mock-server to be ready..."
for i in $(seq 1 60); do
    if curl -sS -o /dev/null "${api_base_path}/tenants" 2>/dev/null; then
        echo "api-mock-server is ready."
        break
    fi
    if [ "$i" -eq 60 ]; then
        echo "api-mock-server did not become ready in time." >&2
        exit 1
    fi
    sleep 1
done

# Register the device with the api-mock-server, attaching the TPM public key
# so the orchestrator's signed requests can be verified.
echo "Registering tenant and device with api-mock-server..."
curl -sS -X POST "${api_base_path}/tenants" \
    -H "Content-Type: application/json" \
    -H "Authorization: Bearer ${jwt}" \
    -d '{ "name": "edge-ipc", "description": "Lima VM used for local end-to-end testing" }'
echo

readonly device_uuid="00000000-0000-0000-0000-000000000001"
curl -sS -X POST "${api_base_path}/devices" \
    -H "Content-Type: application/json" \
    -H "Authorization: Bearer ${jwt}" \
    -d "{ \"uuid\": \"${device_uuid}\", \"serial_number\": \"bla\", \"tenant_id\": 1 }"
echo

tpm_pubkey_json="$(sed -z 's/\n/\\n/g' /tmp/my_tpm_pubkey.pem)"
curl -i -X PUT "${api_base_path}/devices/1" \
    -H "Content-Type: application/json" \
    -H "Authorization: Bearer ${jwt}" \
    -d "{ \"uuid\": \"${device_uuid}\", \"serial_number\": \"bla\", \"tenant_id\": 1, \"public_key\": \"${tpm_pubkey_json}\" }"
echo

# Build the orchestrator binary on the host and copy it into the VM so the
# VM runs whatever local changes are currently checked out, instead of
# whatever got baked into the image at build time.
echo "Building orchestrator binary on host..."
cargo build --package amos-orchestrator

echo "Copying orchestrator binary into the VM..."
limactl copy target/debug/amos-orchestrator edge-ipc:/tmp/amos-orchestrator
# /usr is part of the bootc image's read-only composefs root, so the
# image-baked binary under /usr/local/bin (or /usr/libexec, depending on
# which image version is deployed) can't be overwritten in place. Install
# into /var/usrlocal/bin instead, which the Lima template already
# provisions as a writable stand-in for /usr/local (see edge-ipc.yaml).
limactl shell edge-ipc -- sudo install -m 0755 /tmp/amos-orchestrator /var/usrlocal/bin/amos-orchestrator

# limactl copy leaves the file with whatever SELinux context new files get
# by default (usr_t), not the bin_t the original, image-baked binary had,
# which a confined systemd service is not allowed to exec. Relabel it
# according to policy instead of disabling enforcement. (SELinux has a
# path equivalence between /var/usrlocal and /usr/local, so this still
# resolves to bin_t.)
limactl shell edge-ipc -- sudo restorecon -v /var/usrlocal/bin/amos-orchestrator

# Point orchestrator.service at the binary we just copied in via a
# drop-in override (written to the writable /etc), rather than relying on
# whatever ExecStart path was baked into the image.
echo "Overriding orchestrator.service to run the freshly copied binary..."
limactl shell edge-ipc -- sudo mkdir -p /etc/systemd/system/orchestrator.service.d
limactl shell edge-ipc -- sudo tee /etc/systemd/system/orchestrator.service.d/override.conf >/dev/null <<'EOF'
[Service]
ExecStart=
ExecStart=/var/usrlocal/bin/amos-orchestrator --config /etc/amos/config.toml
EOF
limactl shell edge-ipc -- sudo systemctl daemon-reload

# Restart the orchestrator inside the VM so it picks up the newly registered
# device/public key and the freshly copied binary.
echo "Restarting orchestrator inside the VM..."
limactl shell edge-ipc -- sudo systemctl restart orchestrator

echo "All services are up. Press Ctrl+C to stop the api-mock-server and TimescaleDB (the VM keeps running)."
wait "$server_pid"
