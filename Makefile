.PHONY: setup setup-template setup-hooks help image image-amd64 image-arm64 image-clean _image-build pull-image pull-image-amd64 pull-image-arm64 _image-pull iso iso-amd64 iso-arm64 iso-clean _iso-build demo-edge demo-server

IMAGE         ?= localhost/amos-edge:dev
DIST_DIR      ?= $(CURDIR)/dist
TMP_DIR       ?= /tmp/amos2026ss01-zero-downtime-linux-updates
IMAGE_BUILDER ?= ghcr.io/osbuild/image-builder-cli:latest
HOST_ARCH     := $(shell uname -m | sed -e s/arm64/aarch64/ -e s/amd64/x86_64/)
DOCKER_ARCH    = $(subst aarch64,arm64,$(subst x86_64,amd64,$(ARCH)))

# dev-deploy: name of the running Lima VM and a scratch path inside the guest
# used as the drop point for the freshly built binary, which `limactl copy`
# (scp/sftp over the VM's SSH connection) uploads there.
DEV_VM        ?= edge-ipc
DEV_VM_TMP    ?= /tmp
RUST_VERSION  ?= 1.95
# Per-arch suffix (-amd64 / -arm64) is appended at use site to keep cross-arch
# builds from clobbering each other in podman's local storage.
RUST_BUILDER  ?= localhost/amos-rust-builder:$(RUST_VERSION)

# settings for demo
# SERVER_IP/SERVER_PORT compose the orchestrator's cloud_url (the /v1 prefix is
# added automatically); VM_UUID/VM_SERIAL are the SMBIOS values the emulated
# device presents (VM_UUID is its device id).
SERVER_IP     ?= host.lima.internal
SERVER_PORT   ?= 8080
VM_UUID       ?= 00000000-0000-0000-0000-000000000001
VM_SERIAL     ?= AMOS-TEST-001

# Prebuilt disk image published by .github/workflows/disk-image.yml as an OCI
# artifact (each tag bundles both <name>.raw.xz and <name>.qcow2.xz).
# PULL_REF is required: pass the branch/release tag (without the arch suffix),
# e.g. `make pull-image PULL_REF=feat-create-test-setup` pulls feat-create-test-setup-amd64.
GHCR_DISK     ?= ghcr.io/amosproj/amos2026ss01-zero-downtime-linux-updates-disk
PULL_REF      ?=

# Installer ISO (see the `iso` targets below). Unlike the disk flow, the ISO is
# produced by the older `bootc-image-builder` container: its installer +
# kickstart support (the `anaconda-iso` type, auto-reading /config.toml) is the
# best-documented path. Both flows consume the same bootc image, so this is fine.
ISO_CONFIG    ?= bootc-build/iso/config.toml
BIB           ?= quay.io/centos-bootc/bootc-image-builder:latest
ISO_TYPE      ?= anaconda-iso
FS            ?= ext4

# Dev-vs-prod image switch (Containerfile build-arg). When true, bakes in the
# permissive dev signature policy and drops the /etc/amos-dev-image marker.
# Defaults to false so CI builds (and any "real IPC" artifact) are prod by
# default; pass DEV_MODE=true for local VM testing.
DEV_MODE      ?= false

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
		--platform linux/$(DOCKER_ARCH) \
		--build-arg DEV_MODE=$(DEV_MODE) \
		-f bootc-build/Containerfile -t $(IMAGE) .
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
	bin=$$target/debug/amos-orchestrator; \
	if [ "$(BUILD)" = native ]; then \
	  command -v cargo >/dev/null 2>&1 || { echo "Error: 'cargo' not found. Install Rust or use 'make dev-deploy-container'." >&2; exit 1; }; \
	  echo ">>> Building amos-orchestrator natively -> $$target/release/"; \
	  cargo build --package amos-orchestrator --target-dir $$target; \
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
	    cargo build --package amos-orchestrator \
	      --target-dir /workspace/target/dev-vm-$$vm_arch; \
	  bin=$$target/debug/amos-orchestrator; \
	fi; \
	echo ">>> Uploading $$bin to $(DEV_VM):$(DEV_VM_TMP)/amos-orchestrator.new"; \
	limactl copy $$bin $(DEV_VM):$(DEV_VM_TMP)/amos-orchestrator.new; \
	echo ">>> Installing into $(DEV_VM):/var/usrlocal/bin/amos-orchestrator and restarting"; \
	limactl shell $(DEV_VM) -- sudo install -m755 $(DEV_VM_TMP)/amos-orchestrator.new /var/usrlocal/bin/amos-orchestrator; \
	limactl shell $(DEV_VM) -- sudo systemctl daemon-reload; \
	limactl shell $(DEV_VM) -- sudo systemctl restart orchestrator.service; \
	echo ">>> Deployed. Tail logs: limactl shell $(DEV_VM) -- journalctl -u orchestrator.service -f"

# e2e: spin up a disposable Lima VM, deploy the current orchestrator build
# into it via dev-deploy, run the full scripts/tests/*.sh suite against it,
# then delete the VM again so every run starts from a clean slate.
e2e: ## Create a fresh Lima VM, deploy the local orchestrator build, run the e2e suite, then tear the VM down
	# FIXME: cargo build
	@set -eu; \
	echo ">>> Recreating Lima VM $(DEV_VM) from scratch"; \
	limactl stop $(DEV_VM) -f >/dev/null 2>&1 || true; \
	limactl delete $(DEV_VM) -f >/dev/null 2>&1 || true; \
	limactl create -y --name $(DEV_VM) dev-env/lima/edge-ipc.yaml; \
	trap 'echo ">>> Deleting Lima VM $(DEV_VM)"; limactl stop $(DEV_VM) -f >/dev/null 2>&1 || true; limactl delete $(DEV_VM) -f >/dev/null 2>&1 || true' EXIT; \
	cd scripts && ./e2e_run_all.sh

# demo-edge: like e2e, but with nothing on the host -- no mock server, no
# TimescaleDB, no tests. Prompts interactively for the VM name, device uuid,
# device serial and cloud url, each with a default you can accept by pressing
# enter.
demo-edge: ## Boot a fresh Lima VM with just the orchestrator (no mock server/DB); prompts for VM name/device uuid/serial/cloud url
	@set -eu; \
	printf "VM name [%s]: " "$(DEV_VM)"; read vm_name; vm_name=$${vm_name:-$(DEV_VM)}; \
	printf "Device UUID [%s]: " "$(VM_UUID)"; read device_uuid; device_uuid=$${device_uuid:-$(VM_UUID)}; \
	printf "Device serial [%s]: " "$(VM_SERIAL)"; read device_serial; device_serial=$${device_serial:-$(VM_SERIAL)}; \
	printf "Cloud URL [http://%s:%s/v1]: " "$(SERVER_IP)" "$(SERVER_PORT)"; read cloud_url; cloud_url=$${cloud_url:-http://$(SERVER_IP):$(SERVER_PORT)/v1}; \
	echo ">>> Recreating Lima VM $$vm_name from scratch"; \
	limactl stop $$vm_name -f >/dev/null 2>&1 || true; \
	limactl delete $$vm_name -f >/dev/null 2>&1 || true; \
	limactl create -y --name $$vm_name dev-env/lima/edge-ipc.yaml; \
	cd scripts && \
	  VM_NAME="$$vm_name" \
	  DEVICE_UUID="$$device_uuid" \
	  DEVICE_SERIAL="$$device_serial" \
	  CLOUD_URL="$$cloud_url" \
	  ./dev_vm_run.sh

# demo-server: same full stack as e2e (mock cloud API + TimescaleDB + edge VM +
# orchestrator), primed with a registered device, but instead of running the
# test suite it leaves everything running so you can send your own API commands
# (scripts/demo_api.sh) and watch the orchestrator react. Ctrl+C tears it down.
demo-server: ## Bring up the full e2e stack, primed and left running for a live demo (send commands with scripts/demo_api.sh)
	@set -eu; \
	echo ">>> Recreating Lima VM $(DEV_VM) from scratch"; \
	limactl stop $(DEV_VM) -f >/dev/null 2>&1 || true; \
	limactl delete $(DEV_VM) -f >/dev/null 2>&1 || true; \
	limactl create -y --name $(DEV_VM) dev-env/lima/edge-ipc.yaml; \
	cd scripts && ./demo_run.sh

# ---------------------------------------------------------------------------
# Installer ISO for bare-metal IPCs. The ISO embeds our bootc image and
# installs it unattended via the kickstart in $(ISO_CONFIG). Cross-arch builds
# need qemu-user-static registered on the host.
# ---------------------------------------------------------------------------
iso: ARCH ?= $(HOST_ARCH)
iso: _iso-build ## Build installer ISO for host arch into ./dist/bootiso/install.iso

iso-amd64: ARCH := x86_64
iso-amd64: _iso-build ## Build amd64 installer ISO (cross-arch if host is arm64; needs qemu-user-static)

iso-arm64: ARCH := aarch64
iso-arm64: _iso-build ## Build arm64 installer ISO (cross-arch if host is amd64; needs qemu-user-static)

# Build the bootc image (same as the disk flow) into root podman storage, then
# have bootc-image-builder emit the installer ISO embedding it. The save/load
# dance puts the image where the root `sudo podman run` below can read it.
_iso-build:
	@echo ">>> Building installer ISO for $(ARCH)"
	mkdir -p $(TMP_DIR) $(DIST_DIR)
	podman build \
		--platform linux/$(DOCKER_ARCH) \
		--build-arg DEV_MODE=$(DEV_MODE) \
		-f bootc-build/Containerfile -t $(IMAGE) .
	podman save --format oci-archive -o $(TMP_DIR)/amos-edge.tar $(IMAGE)
	sudo podman load -i $(TMP_DIR)/amos-edge.tar
	sudo podman run --rm --privileged --pull=newer \
		--security-opt label=type:unconfined_t \
		-v $(CURDIR)/$(ISO_CONFIG):/config.toml:ro \
		-v $(DIST_DIR):/output \
		-v /var/lib/containers/storage:/var/lib/containers/storage \
		$(BIB) \
		--type $(ISO_TYPE) \
		--config /config.toml \
		--rootfs $(FS) \
		--target-arch $(DOCKER_ARCH) \
		$(IMAGE)
	sudo chown -R $$USER $(DIST_DIR)/bootiso
	@echo "Installer ISO -> $(DIST_DIR)/bootiso/install.iso"

iso-clean: ## Remove built installer ISO artifacts
	rm -rf $(DIST_DIR)/bootiso
