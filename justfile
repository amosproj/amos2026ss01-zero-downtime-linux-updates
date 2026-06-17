# AMOS edge — task runner. `just --list` shows all recipes.

set shell := ["bash", "-cu"]

image_tag     := env_var_or_default("IMAGE",         "localhost/amos-edge:dev")
dist_dir      := env_var_or_default("DIST_DIR",      justfile_directory() / "dist")
tmp_dir       := env_var_or_default("TMP_DIR",       "/tmp/amos2026ss01-zero-downtime-linux-updates")
image_builder := env_var_or_default("IMAGE_BUILDER", "ghcr.io/osbuild/image-builder-cli:latest")
ghcr_disk     := env_var_or_default("GHCR_DISK",     "ghcr.io/amosproj/amos2026ss01-zero-downtime-linux-updates-disk")

host_arch := `uname -m | sed -e s/arm64/aarch64/ -e s/amd64/x86_64/`

# Default: list available recipes.
default:
    @just --list

# Set up local development environment.
setup: setup-template setup-hooks
    @echo ""
    @echo "  Setup complete!"
    @echo ""
    @echo "  - Commit template configured (shows conventional commit format)"
    @echo "  - Git hook installed (auto-adds DCO sign-off)"
    @echo ""
    @echo "  You can now commit normally. Use 'git commit -s' for explicit sign-off,"
    @echo "  or rely on the hook to add it automatically."
    @echo ""

# Configure the commit message template.
setup-template:
    @git config --local commit.template .gitmessage
    @echo "  Commit template configured."

# Install git hooks.
setup-hooks:
    @cp scripts/hooks/prepare-commit-msg .git/hooks/prepare-commit-msg
    @chmod +x .git/hooks/prepare-commit-msg
    @echo "  Git hooks installed."

# Build bootc disk image. arch: amd64|arm64 (also accepts x86_64|aarch64). format: qcow2|raw|all.
image arch=host_arch format="all":
    #!/usr/bin/env bash
    set -euo pipefail
    case "{{arch}}" in
      amd64|x86_64)  arch_uname=x86_64;  arch_docker=amd64 ;;
      arm64|aarch64) arch_uname=aarch64; arch_docker=arm64 ;;
      *) echo "unsupported arch: {{arch}} (expected amd64|arm64|x86_64|aarch64)" >&2; exit 1 ;;
    esac
    case "{{format}}" in
      qcow2|raw|all) ;;
      *) echo "unsupported format: {{format}} (expected qcow2|raw|all)" >&2; exit 1 ;;
    esac
    echo ">>> Building disk image for ${arch_uname} (format: {{format}})"
    mkdir -p {{tmp_dir}} {{dist_dir}}/qcow2 {{dist_dir}}/image
    podman build \
        --platform "linux/${arch_docker}" \
        --build-arg DEV_MODE=true \
        -f rootc-build/Containerfile -t {{image_tag}} .
    podman save --format oci-archive -o {{tmp_dir}}/amos-edge.tar {{image_tag}}
    sudo podman load -i {{tmp_dir}}/amos-edge.tar
    ib_run() {
        sudo podman run --rm --privileged --pull=newer \
            --security-opt label=type:unconfined_t \
            -v {{dist_dir}}:/output \
            -v /var/lib/containers/storage:/var/lib/containers/storage \
            {{image_builder}} build \
                --bootc-ref {{image_tag}} \
                --bootc-default-fs ext4 \
                --arch "${arch_uname}" \
                --output-dir /output \
                "$1"
    }
    if [[ "{{format}}" == "qcow2" || "{{format}}" == "all" ]]; then
        ib_run qcow2
        mv {{dist_dir}}/*.qcow2 {{dist_dir}}/qcow2/disk.qcow2
    fi
    if [[ "{{format}}" == "raw" || "{{format}}" == "all" ]]; then
        ib_run raw
        mv {{dist_dir}}/*.raw {{dist_dir}}/image/disk.raw
    fi
    sudo chown -R "$USER" {{dist_dir}}

# Download prebuilt disk image from GHCR for the given branch/tag.
# arch: amd64|arm64 (also accepts x86_64|aarch64). format: qcow2|raw (defaults to qcow2 for amd64, raw for arm64).
pull-image ref arch=host_arch format="":
    #!/usr/bin/env bash
    set -euo pipefail
    case "{{arch}}" in
      amd64|x86_64)  gharch=amd64; default_fmt=qcow2 ;;
      arm64|aarch64) gharch=arm64; default_fmt=raw   ;;
      *) echo "unsupported arch: {{arch}} (expected amd64|arm64|x86_64|aarch64)" >&2; exit 1 ;;
    esac
    fmt="{{format}}"
    [ -z "${fmt}" ] && fmt="${default_fmt}"
    case "${fmt}" in
      qcow2) dest={{dist_dir}}/qcow2/disk.qcow2 ;;
      raw)   dest={{dist_dir}}/image/disk.raw   ;;
      *) echo "unsupported format: ${fmt} (expected qcow2|raw)" >&2; exit 1 ;;
    esac
    oci_ref="{{ghcr_disk}}:{{ref}}-${gharch}"
    art="amos-edge-{{ref}}-${gharch}.${fmt}.xz"
    echo ">>> Pulling ${oci_ref} (format: ${fmt})"
    mkdir -p {{tmp_dir}} {{dist_dir}}/qcow2 {{dist_dir}}/image
    oras pull -o {{tmp_dir}} "${oci_ref}"
    echo ">>> Decompressing ${art} -> ${dest}"
    xz -dc "{{tmp_dir}}/${art}" > "${dest}"
    echo ">>> Ready: ${dest}"

# Remove locally built disk images.
image-clean:
    rm -rf {{dist_dir}}
    rm -rf {{tmp_dir}}
