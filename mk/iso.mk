# Build ISO image to boot install on bare-metal IPC
#
# Targets:
#   make iso         build an installer ISO for the host arch  -> $(DIST_DIR)/bootiso/install.iso
#    mk
#   make iso-amd64   cross-build an x86_64 installer ISO (native runner recommended)
#   make iso-arm64   cross-build an aarch64 installer ISO (native runner recommended)
#   make iso-clean   remove built ISO artifacts
#
# The ISO embeds our bootc image and installs it unattended via the kickstart in
# rootc-build/iso/config.toml.
#
# NOTE on tooling: the disk-image flow uses `image-builder-cli`. For the ISO we
# use the canonical `bootc-image-builder` container, because its installer +
# kickstart support (the `anaconda-iso` type, auto-reading /config.toml) is the
# best-documented path. Both consume the same bootc image, so this is fine.

ISO_CONFIG ?= rootc-build/iso/config.toml
BIB        ?= quay.io/centos-bootc/bootc-image-builder:latest
ISO_TYPE   ?= anaconda-iso

.PHONY: iso iso-amd64 iso-arm64 iso-clean _iso-build

iso: ## Build installer ISO for host arch into ./dist/bootiso/install.iso
	$(MAKE) _iso-build ARCH=$(ARCH)

iso-amd64: ## Build amd64 installer ISO (cross-arch if host is arm64; needs qemu-user-static)
	$(MAKE) _iso-build ARCH=amd64

iso-arm64: ## Build arm64 installer ISO (cross-arch if host is amd64; needs qemu-user-static)
	$(MAKE) _iso-build ARCH=arm64

# Build the container image (same as the disk flow), then have bootc-image-builder
# emit the installer ISO embedding it.
_iso-build:
	mkdir -p $(DIST_DIR)
	podman build \
		--platform linux/$(ARCH) \
		--build-arg DEV_MODE=true \
		-f rootc-build/Containerfile \
		-t $(IMAGE) .
	sudo podman run --rm --privileged --pull=newer \
		--security-opt label=type:unconfined_t \
		-v $(CURDIR)/$(ISO_CONFIG):/config.toml:ro \
		-v $(CURDIR)/$(DIST_DIR):/output \
		-v /var/lib/containers/storage:/var/lib/containers/storage \
		$(BIB) \
		--type $(ISO_TYPE) \
		--config /config.toml \
		--target-arch $(ARCH) \
		--local $(IMAGE)
	@echo "Installer ISO -> $(DIST_DIR)/bootiso/install.iso"

iso-clean: ## Remove built installer ISO artifacts
	rm -rf $(DIST_DIR)/bootiso
