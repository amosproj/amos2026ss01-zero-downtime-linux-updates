# Cargo builder image used by `make dev-deploy` so the cross-arch orchestrator
# build inherits the system libraries it links against (libtss2 for TPM).
#
# Mirrors the builder stage of rootc-build/Containerfile — keep system deps in
# sync. CI builds the rootc Containerfile directly (no Makefile), so we can't
# share a base image without GHCR plumbing; this file is the dev-side twin.

ARG RUST_VERSION=1.95
FROM docker.io/rust:${RUST_VERSION}-slim

RUN apt-get update \
 && apt-get install -y --no-install-recommends libtss2-dev \
 && rm -rf /var/lib/apt/lists/*
