.PHONY: setup setup-template setup-hooks help image image-amd64 image-arm64 image-clean _image-build pull-image pull-image-amd64 pull-image-arm64 _image-pull dev-deploy

IMAGE         ?= localhost/amos-edge:dev
DIST_DIR      ?= $(CURDIR)/dist
TMP_DIR       ?= /tmp/amos2026ss01-zero-downtime-linux-updates
IMAGE_BUILDER ?= ghcr.io/osbuild/image-builder-cli:latest
HOST_ARCH     := $(shell uname -m | sed -e s/arm64/aarch64/ -e s/amd64/x86_64/)

# dev-deploy: name of the running Lima VM and the host-mounted share used as a
# drop point. The lima yaml mounts host $(LIMA_TMP) at the same path inside the
# VM, so files copied here are visible to the guest without an SSH round-trip.
DEV_VM        ?= edge-ipc
LIMA_TMP      ?= /tmp/lima
RUST_BUILDER  ?= docker.io/rust:1.95-slim

# Prebuilt disk image published by .github/workflows/disk-image.yml as an OCI
# artifact (each tag bundles both <name>.raw.xz and <name>.qcow2.xz).
# PULL_REF is required: pass the branch/release tag (without the arch suffix),
# e.g. `make pull-image PULL_REF=feat-create-test-setup` pulls feat-create-test-setup-amd64.
GHCR_DISK     ?= ghcr.io/amosproj/amos2026ss01-zero-downtime-linux-updates-disk
PULL_REF      ?=

# Shared image-builder-cli invocation; append the image type (qcow2 / raw).
# Writes the disk straight into $(DIST_DIR) via --output-dir.
IB_RUN = sudo podman run --rm --privileged --pull=newer \
	--security-opt label=type:unconfined_t \
	-v $(DIST_DIR):/output \
	-v /var/lib/containers/storage:/var/lib/containers/storage \
	$(IMAGE_BUILDER) build \
		--bootc-ref $(IMAGE) \
		--bootc-default-fs ext4 \
		--arch $(ARCH) \
		--output-dir /output

help: ## Show available targets
	@grep -E '^[a-zA-Z0-9_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | \
		awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-20s\033[0m %s\n", $$1, $$2}'

setup: setup-template setup-hooks ## Set up local development environment
	@echo ""
	@echo "  Setup complete!"
	@echo ""
	@echo "  - Commit template configured (shows conventional commit format)"
	@echo "  - Git hook installed (auto-adds DCO sign-off)"
	@echo ""
	@echo "  You can now commit normally. Use 'git commit -s' for explicit sign-off,"
	@echo "  or rely on the hook to add it automatically."
	@echo ""

setup-template: ## Configure the commit message template
	@git config --local commit.template .gitmessage
	@echo "  Commit template configured."

setup-hooks: ## Install git hooks
	@cp scripts/hooks/prepare-commit-msg .git/hooks/prepare-commit-msg
	@chmod +x .git/hooks/prepare-commit-msg
	@echo "  Git hooks installed."

image: ARCH ?= $(HOST_ARCH)
image: _image-build ## Build bootc disk image (qcow2 + raw) for host arch into ./dist

image-amd64: ARCH := x86_64
image-amd64: _image-build ## Build amd64 disk image (cross-arch if host is arm64; needs qemu-user-static)

image-arm64: ARCH := aarch64
image-arm64: _image-build ## Build arm64 disk image (cross-arch if host is amd64; needs qemu-user-static)

_image-build:
	@for t in podman; do \
	  command -v $$t >/dev/null 2>&1 || { echo "Error: '$$t' not found. Install it with your package manager (e.g. \`brew install $$t\`)." >&2; exit 1; }; \
	done
	@echo ">>> Building disk image for $(ARCH)"
	mkdir -p $(TMP_DIR) $(DIST_DIR)/qcow2 $(DIST_DIR)/image
	podman build \
		--platform linux/$$(echo $(ARCH) | sed -e s/x86_64/amd64/ -e s/aarch64/arm64/) \
		--build-arg DEV_MODE=true \
		-f rootc-build/Containerfile -t $(IMAGE) .
	podman save --format oci-archive -o $(TMP_DIR)/amos-edge.tar $(IMAGE)
	sudo podman load -i $(TMP_DIR)/amos-edge.tar
	$(IB_RUN) qcow2
	$(IB_RUN) raw
	mkdir -p $(DIST_DIR)/qcow2 $(DIST_DIR)/image
	mv $(DIST_DIR)/*.qcow2 $(DIST_DIR)/qcow2/disk.qcow2
	mv $(DIST_DIR)/*.raw   $(DIST_DIR)/image/disk.raw
	sudo chown -R $$USER:$$USER $(DIST_DIR)

pull-image: ARCH ?= $(HOST_ARCH)
pull-image: _image-pull ## Download prebuilt disk image from GHCR for host arch into ./dist

pull-image-amd64: ARCH := x86_64
pull-image-amd64: _image-pull ## Download prebuilt amd64 disk image (qcow2) from GHCR

pull-image-arm64: ARCH := aarch64
pull-image-arm64: _image-pull ## Download prebuilt arm64 disk image (raw) from GHCR

# Lima's `location:` can't consume an OCI/oras ref, so we pull + decompress the
# artifact into the same dist/ paths the local `image` targets write, which the
# edge-ipc.yaml template points at.
#
# Each tag bundles both <name>.raw.xz and <name>.qcow2.xz. `oras pull` would
# fetch both layers; resolve the digest of the one we need and `oras blob
# fetch` just that.
_image-pull:
	@set -eu; \
	for t in oras jq xz; do \
	  command -v $$t >/dev/null 2>&1 || { echo "Error: '$$t' not found. Install it with your package manager (e.g. \`brew install $$t\`)." >&2; exit 1; }; \
	done; \
	if [ -z "$(PULL_REF)" ]; then \
	  echo "PULL_REF is required, e.g. make pull-image PULL_REF=feat-create-test-setup (pulls feat-create-test-setup-amd64)" >&2; \
	  exit 1; \
	fi; \
	case "$(ARCH)" in \
	  aarch64) gharch=arm64; fmt=raw;   dest=$(DIST_DIR)/image/disk.raw ;; \
	  x86_64)  gharch=amd64; fmt=qcow2; dest=$(DIST_DIR)/qcow2/disk.qcow2 ;; \
	  *) echo "unsupported arch: $(ARCH)" >&2; exit 1 ;; \
	esac; \
	ref="$(GHCR_DISK):$(PULL_REF)-$$gharch"; \
	art="amos-edge-$(PULL_REF)-$$gharch.$$fmt.xz"; \
	echo ">>> Pulling $$art from $$ref"; \
	mkdir -p $(TMP_DIR) $(DIST_DIR)/qcow2 $(DIST_DIR)/image; \
	digest=$$(oras manifest fetch "$$ref" | \
	  jq -r --arg name "$$art" '.layers[] | select(.annotations."org.opencontainers.image.title" == $$name) | .digest'); \
	if [ -z "$$digest" ]; then \
	  echo "could not find layer $$art in $$ref" >&2; exit 1; \
	fi; \
	oras blob fetch --output "$(TMP_DIR)/$$art" "$(GHCR_DISK)@$$digest"; \
	echo ">>> Decompressing $$art -> $$dest"; \
	xz -dc "$(TMP_DIR)/$$art" > "$$dest"; \
	echo ">>> Ready: $$dest"

image-clean: ## Remove locally built disk images
	rm -rf $(DIST_DIR)
	rm -rf $(TMP_DIR)

# dev-deploy: cross-build the orchestrator for the running VM's arch, drop it
# through the host-mounted /tmp/lima share, and restart the service. The VM's
# systemd drop-in (10-dev.conf, written by edge-ipc.yaml) points ExecStart at
# /var/usrlocal/bin/amos-orchestrator, so this swaps the binary without
# rebuilding or redeploying the OS image.
#
# We resolve the VM arch from `uname -m` inside the VM (not the host) because
# the host may be macOS arm64/amd64 while the VM may have been started with a
# different --arch, and the orchestrator needs the VM's arch. The build runs
# inside a Linux Rust container at the right platform, so devs don't need a
# Linux cross-toolchain on macOS.
dev-deploy: ## Cross-build orchestrator for running VM and hot-swap+restart the service
	@set -eu; \
	command -v limactl >/dev/null 2>&1 || { echo "Error: 'limactl' not found." >&2; exit 1; }; \
	command -v podman  >/dev/null 2>&1 || { echo "Error: 'podman' not found." >&2; exit 1; }; \
	vm_arch=$$(limactl shell $(DEV_VM) -- uname -m | tr -d '\r'); \
	case "$$vm_arch" in \
	  aarch64) plat=linux/arm64 ;; \
	  x86_64)  plat=linux/amd64 ;; \
	  *) echo "unsupported VM arch: $$vm_arch" >&2; exit 1 ;; \
	esac; \
	target=$(CURDIR)/target/dev-vm-$$vm_arch; \
	echo ">>> Building amos-orchestrator for VM ($$vm_arch, $$plat) -> $$target/release/"; \
	mkdir -p $$target $(LIMA_TMP); \
	podman run --rm --platform $$plat \
	  -v $(CURDIR):/workspace \
	  -w /workspace \
	  $(RUST_BUILDER) \
	  cargo build --release --package amos-orchestrator \
	    --target-dir /workspace/target/dev-vm-$$vm_arch; \
	cp $$target/release/amos-orchestrator $(LIMA_TMP)/amos-orchestrator.new; \
	echo ">>> Installing into $(DEV_VM):/var/usrlocal/bin/amos-orchestrator and restarting"; \
	limactl shell $(DEV_VM) -- sudo install -m755 $(LIMA_TMP)/amos-orchestrator.new /var/usrlocal/bin/amos-orchestrator; \
	limactl shell $(DEV_VM) -- sudo systemctl restart orchestrator.service; \
	echo ">>> Deployed. Tail logs: limactl shell $(DEV_VM) -- journalctl -u orchestrator.service -f"
