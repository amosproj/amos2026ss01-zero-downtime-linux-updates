#!/usr/bin/env bash
#
# Bring demo runs into their start state: boot the nested Lima edge VMs on the
# demo host (via `make demo-edge` over ssh) and register two of the three
# devices of each run with the run's API server. The third edge of each run is
# left unregistered on purpose -- adding it via a pending device registration
# IS the demo; do that live with --register-only (see below).
#
# ALL runs live side by side on ONE OpenStack host VM (default
# "amos-edge-host", prepared with ./prepare_edge_vms.sh). The Lima VMs are
# named <run>-edge-<n>, so `./prepare_edge_demo.sh all` boots all nine of them
# in advance and every run is ready to go the moment the previous one ends --
# there is nothing left to do between runs except the live --register-only
# moment. Booting is sequential (a few minutes per VM), so prepare well before
# the demo. Re-preparing a single run only recreates that run's VMs; the other
# runs keep running undisturbed.
#
# Each run has its own API server, so the same device UUIDs/serial can be
# reused across runs:
#
#   cloud url : http://float-172-017-069-035.cc.rrze.net/<run>/v1
#   edge 1    : 019f4785-419a-7060-bc3c-d71c75099ac2
#   edge 2    : 019f4785-419a-777a-9dba-0a79ba5809ef
#   edge 3    : 019f4785-419a-7aae-b79d-f0ad81510156
#   serial    : BIOS-SERIAL-1337-AMOS-TEST-VM (all VMs; the server matches
#               pending registrations on serial + TPM endorsement key)
#
# Per run and edge this script:
#   1. runs `make demo-edge` on the host (non-interactive without a TTY) with
#      DEV_VM/VM_UUID/VM_SERIAL/CLOUD_URL set, which recreates the Lima VM
#      <run>-edge-<n>, boots it with a fresh emulated TPM (each VM gets its
#      own swtpm) and points the orchestrator at the run's cloud URL,
#   2. for the edges listed in REGISTER_EDGES (default "1 2"): reads the TPM
#      endorsement key out of the Lima VM, creates a pending device
#      registration on the run's API server and waits until the orchestrator
#      has self-registered.
#
# The host must already be prepared (repo + disk image) with
# ./prepare_edge_vms.sh. Re-running this script recreates the Lima VMs with
# fresh TPMs; re-registering a device the server already knows creates a
# duplicate device entry, so reset the affected run's API server first in
# that case.
#
# Usage:
#   ./prepare_edge_demo.sh all [edge-num ...]              prepare every run
#   ./prepare_edge_demo.sh <run> [edge-num ...]        default edges: 1 2 3
#   ./prepare_edge_demo.sh <run> --register-only [edge-num ...]   default: 3
#
#   ./prepare_edge_demo.sh all                   # everything ready to go
#   ./prepare_edge_demo.sh run1 --register-only  # LIVE DEMO: register edge 3
#
# Requirements on this machine: openstack CLI (authenticated), jq (>= 1.6),
# curl, ssh + VPN as for prepare_edge_vms.sh.
#
# Overridable via env: HOST_VM (amos-edge-host),
# API_BASE (http://float-172-017-069-035.cc.rrze.net), VM_SERIAL,
# REGISTER_EDGES ("1 2"), OS_NETWORK (FAU-Intern), SSH_USER (debian),
# SSH_OPTS, REPO_DIR (auto-detected if unset).

set -euo pipefail

script_dir="$(cd "$(dirname "$0")" && pwd)"

HOST_VM="${HOST_VM:-amos-edge-host}"
API_BASE="${API_BASE:-http://float-172-017-069-035.cc.rrze.net}"
VM_SERIAL="${VM_SERIAL:-BIOS-SERIAL-1337-AMOS-TEST-VM}"
REGISTER_EDGES="${REGISTER_EDGES:-1 2}"
OS_NETWORK="${OS_NETWORK:-FAU-Intern}"
SSH_USER="${SSH_USER:-debian}"
SSH_OPTS="${SSH_OPTS:--o BatchMode=yes -o StrictHostKeyChecking=accept-new -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR -o ConnectTimeout=10}"
REPO_DIR="${REPO_DIR:-}"

SSH_TIMEOUT=300      # seconds to wait for ssh
REGISTER_TIMEOUT=90  # seconds to wait for the orchestrator to self-register

log() { printf '\033[0;32m>>> %s\033[0m\n' "$*"; }
warn() { printf '\033[0;33m!!! %s\033[0m\n' "$*" >&2; }
die() { printf '\033[0;31mError: %s\033[0m\n' "$*" >&2; exit 1; }

usage() { sed -n '2,59p' "$0" | sed 's/^# \{0,1\}//'; }

RUN_ARG="${1:-${AMOS_DEMO_RUN:-}}"
[ -n "$RUN_ARG" ] || { usage; exit 2; }
shift || true

MODE=prepare
if [ "${1:-}" = "--register-only" ]; then
    MODE=register-only
    shift
fi
EDGES=("$@")
if [ ${#EDGES[@]} -eq 0 ]; then
    if [ "$MODE" = register-only ]; then EDGES=(3); else EDGES=(1 2 3); fi
fi

case "$RUN_ARG" in
    all) RUNS=(run1 run2 run3) ;;
    run1|run2|run3) RUNS=("$RUN_ARG") ;;
    *)  warn "unexpected run name '$RUN_ARG' (expected all|run1|run2|run3); continuing anyway"
        RUNS=("$RUN_ARG") ;;
esac

for t in openstack jq ssh curl; do
    command -v "$t" >/dev/null 2>&1 || die "'$t' not found in PATH"
done

# Dev JWT for the mock API server (exports JWT; everything else it sets is
# harmless here).
# shellcheck source=../tests/common_env.sh
source "$script_dir/../tests/common_env.sh"

uuid_for_edge() {
    case "$1" in
        1) echo 019f4785-419a-7060-bc3c-d71c75099ac2 ;;
        2) echo 019f4785-419a-777a-9dba-0a79ba5809ef ;;
        3) echo 019f4785-419a-7aae-b79d-f0ad81510156 ;;
        *) return 1 ;;
    esac
}

host_ssh() { # host_ssh <remote command string...>
    # shellcheck disable=SC2086  # SSH_OPTS is intentionally word-split
    ssh $SSH_OPTS "${SSH_USER}@${HOST_IP}" "$@"
}

server_ip() {
    openstack server show "$1" -f json -c addresses | jq -r --arg net "$OS_NETWORK" '
        .addresses
        | if type == "object" then .[$net][0]
          else (split("=")[1] | split(",")[0]) end'
}

wait_ssh() {
    local waited=0
    while ! host_ssh true 2>/dev/null; do
        [ "$waited" -lt "$SSH_TIMEOUT" ] || die "$HOST_VM ($HOST_IP) not reachable over ssh after ${SSH_TIMEOUT}s"
        printf '    waiting for ssh on %s (%s)...\n' "$HOST_VM" "$HOST_IP"
        sleep 10; waited=$((waited + 10))
    done
}

detect_repo_dir() {
    host_ssh 'for d in "$HOME"/*/; do
        if [ -f "${d}Makefile" ] && [ -f "${d}scripts/dev_vm_run.sh" ]; then
            printf "%s\n" "${d%/}"; exit 0
        fi
    done; exit 1'
}

# api <METHOD> <path> [json-body] -> sets RESP_CODE and RESP_BODY.
# Talks to the API server of the run currently in $RUN.
api() {
    local method="$1" path="$2" body="${3:-}"
    local url="${API_BASE}/${RUN}${path}" out
    if [ -n "$body" ]; then
        out="$(curl -sS -X "$method" "$url" \
            -H "Content-Type: application/json" \
            -H "Authorization: Bearer ${JWT}" \
            -d "$body" -w $'\n%{http_code}')"
    else
        out="$(curl -sS -X "$method" "$url" \
            -H "Authorization: Bearer ${JWT}" -w $'\n%{http_code}')"
    fi
    RESP_CODE="${out##*$'\n'}"
    RESP_BODY="${out%$'\n'*}"
}

# Resolve the OpenStack host all edges run on -> sets HOST_IP, HOST_REPO
resolve_host() {
    HOST_IP="$(server_ip "$HOST_VM")"
    [ -n "$HOST_IP" ] && [ "$HOST_IP" != null ] \
        || die "could not determine IP of $HOST_VM (does it exist? run prepare_edge_vms.sh first)"
    wait_ssh
    HOST_REPO="$REPO_DIR"
    if [ -z "$HOST_REPO" ]; then
        HOST_REPO="$(detect_repo_dir)" \
            || die "could not find the repo checkout on $HOST_VM (set REPO_DIR to override)"
    fi
    host_ssh "test -f '$HOST_REPO/dist/qcow2/disk.qcow2' || test -f '$HOST_REPO/dist/image/disk.raw'" \
        || die "$HOST_VM has no disk image in dist/; run prepare_edge_vms.sh first"
}

# resolve_edge <n> -> sets LIMA_VM, EDGE_UUID (for the run in $RUN)
resolve_edge() {
    local n="$1"
    EDGE_UUID="$(uuid_for_edge "$n")" || die "no device UUID defined for edge $n"
    LIMA_VM="${RUN}-edge-${n}"
}

boot_edge() { # uses LIMA_VM/EDGE_UUID set by resolve_edge, CLOUD_URL of $RUN
    log "$LIMA_VM: running make demo-edge on $HOST_VM (uuid: $EDGE_UUID)"
    # No TTY on this ssh call, so demo-edge skips its prompts and takes
    # DEV_VM/VM_UUID/VM_SERIAL/CLOUD_URL from the environment.
    host_ssh "cd '$HOST_REPO' && \
        DEV_VM='$LIMA_VM' \
        VM_UUID='$EDGE_UUID' \
        VM_SERIAL='$VM_SERIAL' \
        CLOUD_URL='$CLOUD_URL' \
        make demo-edge" 2>&1 | sed "s/^/[$LIMA_VM] /"
}

register_edge() { # uses LIMA_VM/EDGE_UUID set by resolve_edge
    log "$LIMA_VM: reading TPM endorsement key from Lima VM"
    local ek_file payload
    ek_file="$(mktemp)"
    # Keep the key in a file (not a variable): command substitution would strip
    # the trailing newline, and the server matches the key byte-for-byte
    # against what the orchestrator sends.
    host_ssh "limactl shell '$LIMA_VM' -- sudo tpm2_readpublic -c 0x81010001 -f pem -o /dev/stdout | openssl rsa -pubin 2>/dev/null" > "$ek_file" \
        || { rm -f "$ek_file"; die "$LIMA_VM: could not read endorsement key (is the Lima VM up?)"; }
    grep -q 'BEGIN PUBLIC KEY' "$ek_file" \
        || { rm -f "$ek_file"; die "$LIMA_VM: could not read endorsement key (is the Lima VM up?)"; }
    payload="$(jq -n --arg sn "$VM_SERIAL" --rawfile ek "$ek_file" \
        '{ serial_number: $sn, endorsement_public_key: $ek }')"
    rm -f "$ek_file"

    # A registered device is assigned to the first tenant, so make sure one exists.
    api GET /v1/tenants
    [ "$RESP_CODE" = 200 ] || die "GET /v1/tenants on $CLOUD_URL failed with $RESP_CODE: $RESP_BODY"
    # The list endpoints wrap results in a page envelope, so count .data --
    # `jq length` on the envelope object would count its keys, never zero.
    if [ "$(jq '.data | length' <<<"$RESP_BODY")" -eq 0 ]; then
        log "Creating demo tenant on $CLOUD_URL"
        api POST /v1/tenants '{ "name": "AMOS Demo", "description": "Demo tenant" }'
        [ "$RESP_CODE" = 201 ] || die "creating tenant failed with $RESP_CODE: $RESP_BODY"
    fi

    log "$LIMA_VM: creating pending device registration (serial: $VM_SERIAL)"
    api POST /v1/pending-device-registrations "$payload"
    [ "$RESP_CODE" = 201 ] || die "creating pending registration failed with $RESP_CODE: $RESP_BODY"

    # Restart the orchestrator so it retries immediately instead of on the
    # next poll, then wait for the success line in its journal.
    log "$LIMA_VM: restarting orchestrator and waiting for self-registration"
    host_ssh "limactl shell '$LIMA_VM' -- sudo systemctl restart orchestrator.service"
    local waited=0
    while ! host_ssh "limactl shell '$LIMA_VM' -- journalctl -u orchestrator.service --since=-5min | grep -q 'Successfully self-registered device'"; do
        [ "$waited" -lt "$REGISTER_TIMEOUT" ] || die "$LIMA_VM did not self-register within ${REGISTER_TIMEOUT}s (check on $HOST_VM: limactl shell $LIMA_VM -- journalctl -u orchestrator.service)"
        printf '    waiting for %s to self-register...\n' "$LIMA_VM"
        sleep 5; waited=$((waited + 5))
    done
    log "$LIMA_VM: registered with $CLOUD_URL"
}

in_register_edges() {
    local n
    for n in $REGISTER_EDGES; do [ "$n" = "$1" ] && return 0; done
    return 1
}

# Fail fast: every run's API server must be up before we start booting VMs.
for RUN in "${RUNS[@]}"; do
    CLOUD_URL="${API_BASE}/${RUN}/v1"
    log "$RUN: cloud url $CLOUD_URL"
    api GET /v1/devices
    [ "$RESP_CODE" = 200 ] || die "API server for $RUN not reachable (GET /v1/devices -> $RESP_CODE): $RESP_BODY"
done

log "Resolving edge host $HOST_VM"
resolve_host
log "$HOST_VM ($HOST_IP): repo checkout at $HOST_REPO"

for RUN in "${RUNS[@]}"; do
    CLOUD_URL="${API_BASE}/${RUN}/v1"
    for n in "${EDGES[@]}"; do
        resolve_edge "$n"
        if [ "$MODE" = prepare ]; then
            boot_edge
            if in_register_edges "$n"; then
                register_edge
            else
                log "$LIMA_VM: left unregistered (demo device; register live with: $0 $RUN --register-only $n)"
            fi
        else
            register_edge
        fi
    done
done

echo
for RUN in "${RUNS[@]}"; do
    CLOUD_URL="${API_BASE}/${RUN}/v1"
    log "$RUN state:"
    api GET /v1/devices
    jq . <<<"$RESP_BODY" 2>/dev/null || printf '%s\n' "$RESP_BODY"
done
