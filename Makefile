.PHONY: setup setup-template setup-hooks help image image-amd64 image-arm64 image-clean _image-build pull-image pull-image-amd64 pull-image-arm64 _image-pull

IMAGE         ?= localhost/amos-edge:dev
DIST_DIR      ?= $(CURDIR)/dist
TMP_DIR       ?= /tmp/amos2026ss01-zero-downtime-linux-updates
IMAGE_BUILDER ?= ghcr.io/osbuild/image-builder-cli:latest
HOST_ARCH     := $(shell uname -m | sed -e s/arm64/aarch64/ -e s/amd64/x86_64/)

# Prebuilt disk image published by .github/workflows/disk-image.yml as an OCI
# artifact (each tag bundles both <name>.raw.xz and <name>.qcow2.xz).
# Override the tag for a pinned build, e.g. `make pull-image PULL_REF=sprint-08-release`.
GHCR_DISK     ?= ghcr.io/amosproj/amos2026ss01-zero-downtime-linux-updates-disk
PULL_REF      ?= main

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
	@echo ">>> Building disk image for $(ARCH)"
	mkdir -p $(TMP_DIR) $(DIST_DIR)/qcow2 $(DIST_DIR)/image
	podman build \
		--platform linux/$$(echo $(ARCH) | sed -e s/x86_64/amd64/ -e s/aarch64/arm64/) \
		-f rootc-build/Containerfile -t $(IMAGE) .
	podman save --format oci-archive -o $(TMP_DIR)/amos-edge.tar $(IMAGE)
	sudo podman load -i $(TMP_DIR)/amos-edge.tar
	$(IB_RUN) qcow2
	$(IB_RUN) raw
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
_image-pull:
	@set -eu; \
	case "$(ARCH)" in \
	  aarch64) gharch=arm64; fmt=raw;   dest=$(DIST_DIR)/image/disk.raw ;; \
	  x86_64)  gharch=amd64; fmt=qcow2; dest=$(DIST_DIR)/qcow2/disk.qcow2 ;; \
	  *) echo "unsupported arch: $(ARCH)" >&2; exit 1 ;; \
	esac; \
	ref="$(GHCR_DISK):$(PULL_REF)-$$gharch"; \
	art="amos-edge-$(PULL_REF)-$$gharch.$$fmt.xz"; \
	echo ">>> Pulling $$ref"; \
	mkdir -p $(TMP_DIR) $(DIST_DIR)/qcow2 $(DIST_DIR)/image; \
	oras pull -o $(TMP_DIR) "$$ref"; \
	echo ">>> Decompressing $$art -> $$dest"; \
	xz -dc "$(TMP_DIR)/$$art" > "$$dest"; \
	echo ">>> Ready: $$dest"

image-clean: ## Remove locally built disk images
	rm -rf $(DIST_DIR)
	rm -rf $(TMP_DIR)
