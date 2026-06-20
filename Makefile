.PHONY: setup setup-template setup-hooks help image image-amd64 image-arm64 image-clean _image-build pull-image pull-image-amd64 pull-image-arm64 _image-pull dev-deploy dev-deploy-container _dev-deploy dev-deploy-native _dev-deploy

IMAGE         ?= localhost/amos-edge:dev
DIST_DIR      ?= $(CURDIR)/dist
TMP_DIR       ?= /tmp/amos2026ss01-zero-downtime-linux-updates
IMAGE_BUILDER ?= ghcr.io/osbuild/image-builder-cli:latest
HOST_ARCH     := $(shell uname -m | sed -e s/arm64/aarch64/ -e s/amd64/x86_64/)

# dev-deploy: name of the running Lima VM and a scratch path inside the guest
# used as the drop point for the freshly built binary, which `limactl copy`
# (scp/sftp over the VM's SSH connection) uploads there.
DEV_VM        ?= edge-ipc
DEV_VM_TMP    ?= /tmp
RUST_VERSION  ?= 1.95
# Per-arch suffix (-amd64 / -arm64) is appended at use site to keep cross-arch
# builds from clobbering each other in podman's local storage.
RUST_BUILDER  ?= localhost/amos-rust-builder:$(RUST_VERSION)

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
	sudo chown -R $$USER $(DIST_DIR)

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

# dev-deploy: build the orchestrator, copy it into the VM with `limactl copy`,
# and restart the service. The VM's systemd drop-in points ExecStart at
# /var/usrlocal/bin/amos-orchestrator, so this swaps the binary without
# rebuilding the OS image.
#
# Two build backends, selected by BUILD:
#   native (default)  - `make dev-deploy`: build with the host's cargo. No
#                       podman, but only builds for the host's own arch, so run
#                       it on a Linux host matching the VM (e.g. the devcontainer).
#   container         - `make dev-deploy-container`: cross-build inside a Linux
#                       Rust container; works on macOS and across arches.
dev-deploy: BUILD := native
dev-deploy: _dev-deploy ## Build orchestrator with the host's cargo (no container) and hot-swap+restart

dev-deploy-container: BUILD := container
dev-deploy-container: _dev-deploy ## Cross-build orchestrator in a container for the running VM and hot-swap+restart

_dev-deploy:
	@set -eu; \
	command -v limactl >/dev/null 2>&1 || { echo "Error: 'limactl' not found." >&2; exit 1; }; \
	vm_arch=$$(limactl shell $(DEV_VM) -- uname -m | tr -d '\r'); \
	case "$$vm_arch" in \
	  aarch64) plat=linux/arm64 ;; \
	  x86_64)  plat=linux/amd64 ;; \
	  *) echo "unsupported VM arch: $$vm_arch" >&2; exit 1 ;; \
	esac; \
	target=$(CURDIR)/target/dev-vm-$$vm_arch; \
	mkdir -p $$target; \
	if [ "$(BUILD)" = native ]; then \
	  command -v cargo >/dev/null 2>&1 || { echo "Error: 'cargo' not found. Install Rust or use 'make dev-deploy-container'." >&2; exit 1; }; \
	  echo ">>> Building amos-orchestrator natively -> $$target/release/"; \
	  cargo build --release --package amos-orchestrator --target-dir $$target; \
	  bin=$$target/release/amos-orchestrator; \
	else \
	  command -v podman >/dev/null 2>&1 || { echo "Error: 'podman' not found." >&2; exit 1; }; \
	  plat_arch=$$(echo $$plat | cut -d/ -f2); \
	  builder_tag=$(RUST_BUILDER)-$$plat_arch; \
	  echo ">>> Ensuring rust builder image $$builder_tag (rust + libtss2-dev)"; \
	  podman build --platform $$plat \
	    --build-arg RUST_VERSION=$(RUST_VERSION) \
	    -f dev-env/rust-builder.Containerfile \
	    -t $$builder_tag dev-env; \
	  echo ">>> Building amos-orchestrator for VM ($$vm_arch, $$plat) -> $$target/release/"; \
	  podman run --rm --platform $$plat \
	    -v $(CURDIR):/workspace \
	    -w /workspace \
	    $$builder_tag \
	    cargo build --release --package amos-orchestrator \
	      --target-dir /workspace/target/dev-vm-$$vm_arch; \
	  bin=$$target/release/amos-orchestrator; \
	fi; \
	echo ">>> Uploading $$bin to $(DEV_VM):$(DEV_VM_TMP)/amos-orchestrator.new"; \
	limactl copy $$bin $(DEV_VM):$(DEV_VM_TMP)/amos-orchestrator.new; \
	echo ">>> Installing into $(DEV_VM):/var/usrlocal/bin/amos-orchestrator and restarting"; \
	limactl shell $(DEV_VM) -- sudo install -m755 $(DEV_VM_TMP)/amos-orchestrator.new /var/usrlocal/bin/amos-orchestrator; \
	limactl shell $(DEV_VM) -- sudo systemctl restart orchestrator.service; \
	echo ">>> Deployed. Tail logs: limactl shell $(DEV_VM) -- journalctl -u orchestrator.service -f"
