#!/usr/bin/env bash
#
# De-personalise a Debian VM so it can be snapshotted into a reusable OpenStack
# (or any clone-friendly) image. Run this LAST, right before you shut the VM
# down and capture the disk — anything you do afterwards re-dirties the image.
#
#   cloud-init clean      forget instance metadata so it re-runs on each clone
#   ssh host keys         removed -> regenerated uniquely on first boot
#   machine-id            truncated (NOT deleted) -> regenerated on next boot
#   apt cache             dropped to shrink the image
#   logs / tmp            wiped so clones start with a clean slate
#   shell history         cleared for the invoking (and root) user
#
# Run as a normal user that has sudo (privileged steps call sudo themselves).
# After this finishes: `sudo shutdown -h now`, then capture the disk.
#
# Skip the destructive machine-id / ssh-key / cloud-init steps with --dry-run
# to preview, or force non-interactive with --yes.

set -euo pipefail

log() { printf '\033[0;32m>>> %s\033[0m\n' "$*"; }
warn() { printf '\033[0;33m!!! %s\033[0m\n' "$*" >&2; }
die() { printf '\033[0;31mError: %s\033[0m\n' "$*" >&2; exit 1; }

DRY_RUN=0
ASSUME_YES=0
for arg in "$@"; do
    case "$arg" in
        -n|--dry-run) DRY_RUN=1 ;;
        -y|--yes)     ASSUME_YES=1 ;;
        -h|--help)
            sed -n '2,20p' "$0" | sed 's/^# \{0,1\}//'
            exit 0 ;;
        *) die "unknown argument: $arg (try --help)" ;;
    esac
done

SUDO=""
if [ "$(id -u)" -ne 0 ]; then
    command -v sudo >/dev/null 2>&1 || die "need root or sudo to clean system files"
    SUDO="sudo"
fi

# --- sanity: is this actually Debian? ----------------------------------------
if [ -r /etc/os-release ]; then
    . /etc/os-release
    if [ "${ID:-}" != "debian" ]; then
        warn "expected Debian (got ID=${ID:-?}); continuing anyway"
    fi
fi

# run <cmd...> : echo it, and actually execute unless --dry-run.
run() {
    if [ "$DRY_RUN" -eq 1 ]; then
        printf '    [dry-run] %s\n' "$*"
    else
        "$@"
    fi
}

if [ "$DRY_RUN" -eq 0 ] && [ "$ASSUME_YES" -eq 0 ]; then
    warn "This wipes logs, ssh host keys, machine-id and cloud-init state on THIS host."
    warn "Only do this on a template VM you are about to image, never on a live system."
    printf 'Continue? [y/N] '
    read -r reply
    case "$reply" in
        y|Y|yes|YES) ;;
        *) die "aborted" ;;
    esac
fi

# --- cloud-init: forget this instance so it re-runs on clones ----------------
if command -v cloud-init >/dev/null 2>&1; then
    log "Cleaning cloud-init state (re-runs on next boot)"
    run $SUDO cloud-init clean --logs
else
    warn "cloud-init not installed; skipping (fine if the image doesn't use it)"
fi

# --- ssh host keys: regenerated uniquely per clone ---------------------------
# Leaving these in place would make every clone share the same host identity.
log "Removing ssh host keys (regenerated on first boot)"
run $SUDO sh -c 'rm -f /etc/ssh/ssh_host_*'

# --- machine-id: truncate, do NOT delete -------------------------------------
# systemd regenerates an empty machine-id on next boot; deleting the file
# outright can break early boot, so truncate to zero bytes instead.
log "Truncating /etc/machine-id (regenerated on next boot)"
run $SUDO truncate -s 0 /etc/machine-id
if [ -e /var/lib/dbus/machine-id ] && [ ! -L /var/lib/dbus/machine-id ]; then
    run $SUDO truncate -s 0 /var/lib/dbus/machine-id
fi

# --- apt cache: shrink the image ---------------------------------------------
log "Cleaning apt cache"
run $SUDO apt-get clean

# --- logs / tmp: clean slate for clones --------------------------------------
log "Wiping logs and temp dirs"
run $SUDO sh -c 'rm -rf /var/log/* /tmp/* /var/tmp/*'

# --- shell history -----------------------------------------------------------
log "Clearing shell history"
run sh -c 'cat /dev/null > "$HOME/.bash_history"'
# root has its own history when the privileged steps ran under sudo.
run $SUDO sh -c 'cat /dev/null > /root/.bash_history 2>/dev/null || true'
unset HISTFILE 2>/dev/null || true

# --- lima and podman: clean --------------------------------------
limactl prune
podman system prune -af

rm -rf /tmp/emulated-tpm

echo
if [ "$DRY_RUN" -eq 1 ]; then
    log "Dry run complete — nothing was changed."
else
    log "Done. The VM is de-personalised."
    cat <<'EOF'

Next steps:
  1. Shut the VM down:   sudo shutdown -h now
  2. Capture the disk into an OpenStack image (e.g. `openstack image create`).
EOF
fi
