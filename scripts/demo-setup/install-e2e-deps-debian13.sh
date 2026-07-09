#!/usr/bin/env bash
#
# Install everything needed to run the e2e suite (`make e2e`) on a fresh
# Debian 13 (trixie) VM.
#
#   Lima + QEMU + OVMF   boot the (UEFI) bootc image           -> limactl, qemu, ovmf
#   swtpm                emulated TPM the orchestrator uses    -> apt: swtpm swtpm-tools
#   podman               TimescaleDB + rust-builder containers -> apt: podman, uidmap, passt
#   cargo/rustc          native build of orchestrator+mock srv -> rustup (Makefile pins 1.95)
#   libtss2-dev,pkgcfg   the orchestrator links libtss2        -> apt: libtss2-dev pkg-config
#   oras, jq, xz         `make pull-image` (prebuilt disk)     -> oras (upstream), apt: jq xz-utils
#   curl, make, git      test scripts / build orchestration    -> apt
#   fzf                  fuzzy finder + Ctrl+R history search  -> apt: fzf
#   zellij               terminal multiplexer                  -> zellij (upstream)
#
# Run as a normal user that has sudo (do NOT `sudo bash` this — rustup then
# installs Rust for root instead of you). Privileged steps call sudo themselves.
#
# Overridable via env: RUST_VERSION, LIMA_VERSION, ORAS_VERSION, ZELLIJ_VERSION.

set -euo pipefail

# Keep the pinned Rust in sync with the Makefile's RUST_VERSION default.
RUST_VERSION="${RUST_VERSION:-1.95}"
# Empty => resolve the latest release from GitHub. Set e.g. LIMA_VERSION=v1.2.1
# to pin. The *_FALLBACK values are only used if the GitHub API is unreachable.
LIMA_VERSION="${LIMA_VERSION:-}"
ORAS_VERSION="${ORAS_VERSION:-}"
ZELLIJ_VERSION="${ZELLIJ_VERSION:-}"
LAZYGIT_VERSION="${LAZYGIT_VERSION:-}"
LIMA_FALLBACK="v1.0.0"
ORAS_FALLBACK="v1.2.0"
ZELLIJ_FALLBACK="v0.43.1"
LAZYGIT_FALLBACK="v0.63.0"

log() { printf '\033[0;32m>>> %s\033[0m\n' "$*"; }
warn() { printf '\033[0;33m!!! %s\033[0m\n' "$*" >&2; }
die() { printf '\033[0;31mError: %s\033[0m\n' "$*" >&2; exit 1; }

SUDO=""
if [ "$(id -u)" -ne 0 ]; then
    command -v sudo >/dev/null 2>&1 || die "need root or sudo to install packages"
    SUDO="sudo"
fi

# --- sanity: is this actually Debian 13? -------------------------------------
if [ -r /etc/os-release ]; then
    . /etc/os-release
    if [ "${ID:-}" != "debian" ] || [ "${VERSION_ID:-}" != "13" ]; then
        warn "expected Debian 13 (got ID=${ID:-?} VERSION_ID=${VERSION_ID:-?}); continuing anyway"
    fi
fi

arch="$(uname -m)"          # x86_64 | aarch64
case "$arch" in
    x86_64)  goarch=amd64 ;;
    aarch64) goarch=arm64 ;;
    *) die "unsupported arch: $arch" ;;
esac

# --- apt packages ------------------------------------------------------------
log "Installing apt packages"
export DEBIAN_FRONTEND=noninteractive
$SUDO apt-get update -y
$SUDO apt-get install -y --no-install-recommends \
    ca-certificates curl git jq make xz-utils \
    build-essential pkg-config libtss2-dev \
    qemu-system-x86 qemu-utils ovmf \
    swtpm swtpm-tools \
    podman \
    uidmap passt nftables \
    fzf \
    htop

# --- fzf Ctrl+R shell integration --------------------------------------------
# Debian's fzf ships key bindings but doesn't wire them into the shell. Enable
# Ctrl+R (history search), Ctrl+T and Alt+C for interactive bash sessions.
BASHRC="$HOME/.bashrc"
if ! grep -q 'fzf --bash' "$BASHRC" 2>/dev/null; then
    log "Enabling fzf key bindings (Ctrl+R) in $BASHRC"
    # shellcheck disable=SC2016  # write $(fzf --bash) literally, expand at shell startup
    printf '\n# fzf key bindings + completion (Ctrl+R history search)\ncommand -v fzf >/dev/null 2>&1 && eval "$(fzf --bash)"\n' >> "$BASHRC"
fi

# --- Rust via rustup ---------------------------------------------------------
# Debian's rustc is older than the pinned toolchain, so use rustup. Installed
# for the current (non-root) user under ~/.cargo.
if command -v rustup >/dev/null 2>&1; then
    log "rustup present; ensuring Rust $RUST_VERSION is installed and default"
    rustup toolchain install "$RUST_VERSION"
    rustup default "$RUST_VERSION"
else
    log "Installing rustup + Rust $RUST_VERSION"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
        | sh -s -- -y --default-toolchain "$RUST_VERSION" --profile minimal
fi
# Make cargo visible for the verification below (and hint the user for later).
# shellcheck disable=SC1090
[ -r "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"

# --- limactl (Lima) ----------------------------------------------------------
# Not packaged for Debian; grab the upstream release tarball, which unpacks
# into bin/ + share/ under /usr/local.
if command -v limactl >/dev/null 2>&1; then
    log "limactl present: $(limactl --version 2>/dev/null | head -n1)"
else
    tag="$LIMA_VERSION"
    if [ -z "$tag" ]; then
        tag="$(curl -fsSL https://api.github.com/repos/lima-vm/lima/releases/latest \
            | jq -r .tag_name 2>/dev/null)" || tag=""
        if [ -z "$tag" ] || [ "$tag" = null ]; then tag="$LIMA_FALLBACK"; fi
    fi
    ver="${tag#v}"
    url="https://github.com/lima-vm/lima/releases/download/${tag}/lima-${ver}-Linux-${arch}.tar.gz"
    log "Installing Lima $tag into /usr/local"
    tmp="$(mktemp -d)"
    curl -fsSL "$url" -o "$tmp/lima.tar.gz"
    $SUDO tar -C /usr/local -xzf "$tmp/lima.tar.gz"
    rm -rf "$tmp"
fi

# --- oras --------------------------------------------------------------------
# Not packaged for Debian; single static binary from the upstream tarball.
if command -v oras >/dev/null 2>&1; then
    log "oras present: $(oras version 2>/dev/null | head -n1)"
else
    tag="$ORAS_VERSION"
    if [ -z "$tag" ]; then
        tag="$(curl -fsSL https://api.github.com/repos/oras-project/oras/releases/latest \
            | jq -r .tag_name 2>/dev/null)" || tag=""
        if [ -z "$tag" ] || [ "$tag" = null ]; then tag="$ORAS_FALLBACK"; fi
    fi
    ver="${tag#v}"
    url="https://github.com/oras-project/oras/releases/download/${tag}/oras_${ver}_linux_${goarch}.tar.gz"
    log "Installing oras $tag into /usr/local/bin"
    tmp="$(mktemp -d)"
    curl -fsSL "$url" -o "$tmp/oras.tar.gz"
    $SUDO tar -C /usr/local/bin -xzf "$tmp/oras.tar.gz" oras
    rm -rf "$tmp"
fi

# --- zellij ------------------------------------------------------------------
# Not packaged for Debian 13; single static (musl) binary from the upstream
# release tarball, which unpacks a bare `zellij` binary.
if command -v zellij >/dev/null 2>&1; then
    log "zellij present: $(zellij --version 2>/dev/null | head -n1)"
else
    tag="$ZELLIJ_VERSION"
    if [ -z "$tag" ]; then
        tag="$(curl -fsSL https://api.github.com/repos/zellij-org/zellij/releases/latest \
            | jq -r .tag_name 2>/dev/null)" || tag=""
        if [ -z "$tag" ] || [ "$tag" = null ]; then tag="$ZELLIJ_FALLBACK"; fi
    fi
    url="https://github.com/zellij-org/zellij/releases/download/${tag}/zellij-${arch}-unknown-linux-musl.tar.gz"
    log "Installing zellij $tag into /usr/local/bin"
    tmp="$(mktemp -d)"
    curl -fsSL "$url" -o "$tmp/zellij.tar.gz"
    $SUDO tar -C /usr/local/bin -xzf "$tmp/zellij.tar.gz" zellij
    rm -rf "$tmp"
fi

# --- lazygit ------------------------------------------------------------------
# Not packaged for Debian 13
if command -v lazygit >/dev/null 2>&1; then
    log "lazygit present: $(lazygit --version 2>/dev/null | head -n1)"
else
    tag="$LAZYGIT_VERSION"
    if [ -z "$tag" ]; then
        tag="$(curl -fsSL https://api.github.com/repos/jesseduffield/lazygit/releases/latest \
            | jq -r .tag_name 2>/dev/null)" || tag=""
        if [ -z "$tag" ] || [ "$tag" = null ]; then tag="$LAZYGIT_FALLBACK"; fi
    fi
    ver="${tag#v}"
    url="https://github.com/jesseduffield/lazygit/releases/download/${tag}/lazygit_${ver}_linux_${goarch}.tar.gz"
    log "Installing lazygit $tag into /usr/local/bin"
    tmp="$(mktemp -d)"
    curl -fsSL "$url" -o "$tmp/lazygit.tar.gz"
    $SUDO tar -C /usr/local/bin -xzf "$tmp/lazygit.tar.gz" lazygit
    rm -rf "$tmp"
fi




# --- other ------------------------------------------------------------------

target_user="${SUDO_USER:-$(id -un)}"
log "Adding $target_user to kvm group (for qemu)"
if getent group kvm >/dev/null 2>&1; then
    $SUDO usermod -aG kvm "$target_user"
else
    warn "kvm group not found; skipping usermod"
fi

containers_conf="$HOME/.config/containers/containers.conf"
mkdir -p "$(dirname "$containers_conf")"
if [ -f "$containers_conf" ]; then
    if grep -Eq '^[[:space:]]*cgroup_manager[[:space:]]*=[[:space:]]*"cgroupfs"' "$containers_conf"; then
        log "containers.conf already configures cgroup_manager=\"cgroupfs\""
    else
        warn "$containers_conf exists; set [engine] cgroup_manager=\"cgroupfs\" manually"
    fi
else
    log "Creating $containers_conf for podman cgroup manager"
    cat >"$containers_conf" <<'EOF'
[engine]
cgroup_manager = "cgroupfs"
EOF
fi

# --- summary -----------------------------------------------------------------
echo
log "Installed tool versions:"
check() {
    if command -v "$1" >/dev/null 2>&1; then
        printf '  %-10s %s\n' "$1" "$("${@:2}" 2>&1 | head -n1)"
    else
        printf '  %-10s MISSING\n' "$1"
    fi
}
check cargo    cargo --version
check limactl  limactl --version
check qemu-system-x86_64 qemu-system-x86_64 --version
check swtpm    swtpm --version
check podman   podman --version
check oras     oras version
check jq       jq --version
check fzf      fzf --version
check zellij   zellij --version
check lazygit   lazygit --version

cat <<'EOF'

Done. Next steps:
  1. Get a disk image, e.g.:           make pull-image PULL_REF=main
  2. Build the host-side binaries:      cargo build            (builds amos-api-mock-server)
  3. Run the suite:                     make e2e
EOF
