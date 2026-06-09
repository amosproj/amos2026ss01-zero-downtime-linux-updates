.PHONY: setup setup-template setup-hooks help image image-clean

IMAGE_TAG ?= localhost/amos-edge:dev
DIST_DIR  ?= $(CURDIR)/dist

help: ## Show available targets
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | \
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

image: ## Build bootc disk image (qcow2 + raw) into ./dist
	podman build -f rootc-build/Containerfile -t $(IMAGE_TAG) .
	podman save --format oci-archive -o /tmp/amos-edge.tar $(IMAGE_TAG)
	mkdir -p $(DIST_DIR)
	sudo podman load -i /tmp/amos-edge.tar
	sudo podman run --rm --privileged --pull=missing \
		--security-opt label=type:unconfined_t \
		-v $(DIST_DIR):/output \
		-v /var/lib/containers/storage:/var/lib/containers/storage \
		quay.io/centos-bootc/bootc-image-builder:latest \
		--type qcow2 --type raw \
		--rootfs ext4 \
		$(IMAGE_TAG)
	sudo chown -R $$USER:$$USER $(DIST_DIR)

image-clean: ## Remove locally built disk images
	rm -rf $(DIST_DIR)
