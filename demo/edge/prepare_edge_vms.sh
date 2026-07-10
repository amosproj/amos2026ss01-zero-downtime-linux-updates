#!/usr/bin/env bash
#
# Provision the OpenStack edge host VM(s) for the demo and bring them up to
# date.
#
# The nested Lima edge VMs of ALL demo runs live on ONE beefy OpenStack host
# VM (booted there in advance by ./prepare_edge_demo.sh all -- 3 runs x 3
# edges = 9 VMs running side by side); this script prepares that host, and
# optionally an identical backup host to fail over to. Hosts are cloned from
# the "amos-edge-base" snapshot, which already contains the repo checkout
# and a (by now outdated) disk image, so for each requested host this script:
#
#   1. creates the server on OpenStack (skipped if it already exists),
#   2. waits until it is ACTIVE and reachable over ssh,
#   3. updates the repo checkout baked into the base image,
#   4. runs `make pull-image PULL_REF=<image-tag>` so the nested edge VMs
#      boot the requested image version.
#
# All remote output is streamed back here, prefixed with the host name.
# Afterwards run ./prepare_edge_demo.sh all to boot the nested Lima edge VMs
# of every run and register them with their run's API server.
#
# Usage:
#   ./prepare_edge_vms.sh <image-tag> [host-vm-name ...]
#
#   ./prepare_edge_vms.sh main                     # prepare amos-edge-host
#   ./prepare_edge_vms.sh sprint-09 amos-edge-host amos-edge-backup
#                                   # prepare the primary and a backup host
#
# <image-tag> is the GHCR disk-image tag passed to `make pull-image` as
# PULL_REF (branch/release tag without the arch suffix, e.g. "main").
#
# Requirements on this machine:
#   - openstack CLI, authenticated (source your openrc / clouds.yaml)
#   - jq, ssh
#   - reachability of the FAU-intern network (VPN) and a ssh private key matching
#     the "amos-developer-keys" keypair loaded (ssh-agent or default key)
#
# Overridable via env:
#   HOST_VM (amos-edge-host; the default host when none are given),
#   OS_IMAGE (amos-edge-base),
#   OS_FLAVOR (SCS-16V-32 -- every nested edge VM takes 2 vCPUs + 4GiB RAM
#              plus its own copy of the disk image)
#   OS_KEY_NAME (amos-developer-keys), OS_NETWORK (FAU-Intern),
#   OS_SECURITY_GROUP (default), SSH_USER (debian), SSH_OPTS,
#   REPO_DIR (auto-detected on the VM if unset),
#   GIT_REF  (git ref to check out; default: fast-forward the current branch.
#             Note the image tag is not always a valid git ref -- GHCR tags
#             have "/" sanitized to "-" -- hence the separate variable.)

set -euo pipefail

OS_IMAGE="${OS_IMAGE:-amos-edge-base}"
OS_FLAVOR="${OS_FLAVOR:-SCS-16V-32}"
OS_KEY_NAME="${OS_KEY_NAME:-amos-developer-keys}"
OS_NETWORK="${OS_NETWORK:-FAU-Intern}"
OS_SECURITY_GROUP="${OS_SECURITY_GROUP:-default}"
SSH_USER="${SSH_USER:-debian}"
# Host keys are regenerated per clone and private IPs get recycled between
# runs, so don't pollute (or trip over) the local known_hosts.
SSH_OPTS="${SSH_OPTS:--o BatchMode=yes -o StrictHostKeyChecking=accept-new -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR -o ConnectTimeout=10}"
REPO_DIR="${REPO_DIR:-}"
GIT_REF="${GIT_REF:-}"

ACTIVE_TIMEOUT=600   # seconds to wait for the server to become ACTIVE
SSH_TIMEOUT=600      # seconds to wait for ssh to come up

log() { printf '\033[0;32m>>> %s\033[0m\n' "$*"; }
warn() { printf '\033[0;33m!!! %s\033[0m\n' "$*" >&2; }
die() { printf '\033[0;31mError: %s\033[0m\n' "$*" >&2; exit 1; }

usage() { sed -n '2,54p' "$0" | sed 's/^# \{0,1\}//'; }

IMAGE_TAG="${1:-}"
[ -n "$IMAGE_TAG" ] || { usage; exit 2; }
shift
HOSTS=("$@")
[ ${#HOSTS[@]} -gt 0 ] || HOSTS=("${HOST_VM:-amos-edge-host}")

for t in openstack jq ssh; do
    command -v "$t" >/dev/null 2>&1 || die "'$t' not found in PATH"
done

vm_ssh() { # vm_ssh <ip> <remote command string...>
    local ip="$1"; shift
    # shellcheck disable=SC2086  # SSH_OPTS is intentionally word-split
    ssh $SSH_OPTS "${SSH_USER}@${ip}" "$@"
}

server_exists() {
    openstack server show "$1" -f value -c name >/dev/null 2>&1
}

server_status() {
    openstack server show "$1" -f value -c status
}

# Private IP on $OS_NETWORK. Handles both the dict (newer openstackclient)
# and the legacy "net=ip, ip2" string format of the addresses field.
server_ip() {
    openstack server show "$1" -f json -c addresses | jq -r --arg net "$OS_NETWORK" '
        .addresses
        | if type == "object" then .[$net][0]
          else (split("=")[1] | split(",")[0]) end'
}

wait_active() {
    local vm="$1" waited=0 status
    while :; do
        status="$(server_status "$vm")"
        case "$status" in
            ACTIVE) return 0 ;;
            ERROR)  die "$vm went into ERROR state" ;;
        esac
        [ "$waited" -lt "$ACTIVE_TIMEOUT" ] || die "$vm not ACTIVE after ${ACTIVE_TIMEOUT}s (status: $status)"
        printf '    %s status: %s, waiting...\n' "$vm" "$status"
        sleep 10; waited=$((waited + 10))
    done
}

wait_ssh() {
    local vm="$1" ip="$2" waited=0
    while ! vm_ssh "$ip" true 2>/dev/null; do
        [ "$waited" -lt "$SSH_TIMEOUT" ] || die "$vm ($ip) not reachable over ssh after ${SSH_TIMEOUT}s"
        printf '    waiting for ssh on %s (%s)...\n' "$vm" "$ip"
        sleep 10; waited=$((waited + 10))
    done
}

# The base image has the repo checked out somewhere under $HOME; find it by
# its layout instead of hardcoding the directory name.
detect_repo_dir() {
    vm_ssh "$1" 'for d in "$HOME"/*/; do
        if [ -f "${d}Makefile" ] && [ -f "${d}scripts/dev_vm_run.sh" ]; then
            printf "%s\n" "${d%/}"; exit 0
        fi
    done; exit 1'
}

prepare_host() {
    local vm="$1"

    if server_exists "$vm"; then
        log "$vm already exists; skipping create"
    else
        log "Creating server $vm (image=$OS_IMAGE flavor=$OS_FLAVOR net=$OS_NETWORK)"
        openstack server create \
            --image "$OS_IMAGE" \
            --flavor "$OS_FLAVOR" \
            --key-name "$OS_KEY_NAME" \
            --security-group "$OS_SECURITY_GROUP" \
            --network "$OS_NETWORK" \
            "$vm" >/dev/null
    fi

    log "Waiting for $vm to become ACTIVE"
    wait_active "$vm"

    local ip
    ip="$(server_ip "$vm")"
    [ -n "$ip" ] && [ "$ip" != null ] || die "could not determine IP of $vm on network $OS_NETWORK"
    log "$vm is ACTIVE at $ip; waiting for ssh"
    wait_ssh "$vm" "$ip"

    local repo="$REPO_DIR"
    if [ -z "$repo" ]; then
        repo="$(detect_repo_dir "$ip")" \
            || die "could not find the repo checkout on $vm (set REPO_DIR to override)"
    fi
    log "$vm: repo checkout at $repo"

    log "$vm: updating repo${GIT_REF:+ (checking out $GIT_REF)}"
    vm_ssh "$ip" "set -e; cd '$repo'
        git fetch --tags --prune origin
        if [ -n '$GIT_REF' ]; then git checkout '$GIT_REF'; fi
        if git symbolic-ref -q HEAD >/dev/null; then
            git pull --ff-only
        else
            echo 'detached HEAD (tag checkout); skipping pull'
        fi" 2>&1 | sed "s/^/[$vm] /"

    log "$vm: pulling disk image (PULL_REF=$IMAGE_TAG)"
    vm_ssh "$ip" "cd '$repo' && make pull-image PULL_REF='$IMAGE_TAG'" 2>&1 | sed "s/^/[$vm] /"

    log "$vm: ready"
}

log "Preparing edge hosts: ${HOSTS[*]} (image tag: $IMAGE_TAG)"
for vm in "${HOSTS[@]}"; do
    prepare_host "$vm"
done

echo
log "All done. Next step (use HOST_VM=<name> for a non-default host):"
echo "  ./prepare_edge_demo.sh all"
