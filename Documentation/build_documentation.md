# Build & Deploy Documentation

## Zero-Downtime Linux Updates — Build, Test & Deployment Guide

This document covers how to build, test, and deploy the project components.

---

## Table of Contents

1. [Repository Structure](#repository-structure)
2. [Prerequisites](#prerequisites)
3. [Initial Setup](#initial-setup)
4. [Building the Project](#building-the-project)
5. [Running Tests](#running-tests)
6. [Running Locally](#running-locally)
7. [Deploying to an Edge Device](#deploying-to-an-edge-device)
8. [Container / bootc Build](#container--bootc-build)
9. [CI / CD](#ci--cd)
10. [Environment Variables Reference](#environment-variables-reference)

---

## Repository Structure

```
amos2026ss01-zero-downtime-linux-updates/
├── Cargo.toml              — Workspace root manifest
├── Makefile                — Developer setup helpers
├── orchestrator/           — Main agent binary (amos-orchestrator)
│   ├── Cargo.toml
│   ├── config.example.toml
│   └── src/
├── common/                 — Shared library crate (amos-common)
│   │
│   ├── Cargo.toml
│   └── src/
├── api-server/        — Development server binary
│   ├── Cargo.toml
│   └── src/
└── bootc-build/            — Container image build files
    ├── Containerfile
    └── orchestrator.service
```

---

## Prerequisites

### For local development

| Tool | Version | Purpose |
|------|---------|---------|
| [Rust](https://rustup.rs/) | stable (≥ 1.80) | Build toolchain |
| `cargo` | (included with Rust) | Package manager & build tool |
| `git` | ≥ 2.x | Version control |
| `make` | any | Developer setup shortcuts |

Additionally the tss2 library is needed for the LSP to work, as the `tss-esapi` crate still needs certain headers to be present that are not included with the crate. For installation, the package name depends on your distribution:

- Debian(like): `libtss2-dev`
- Fedora/RHEL: `tpm2-tss-devel`

Optional (for full integration testing):

- `podman` — container runtime
- `bootc` — OS image tooling
- `rpm-ostree` — OSTree management

### For building container images

- `podman` or `docker` with OCI image support
- Access to a container registry (GHCR)

---

## Initial Setup

After cloning the repository, run the one-time developer setup:

```bash
git clone https://github.com/amosproj/amos2026ss01-zero-downtime-linux-updates.git
cd amos2026ss01-zero-downtime-linux-updates
make setup
```

`make setup` configures:

- A **commit message template** (conventional commits format).
- A **Git hook** that automatically appends the DCO sign-off line to commit messages.

> **Note:** All commits must be signed off (`git commit -s`) per the project's [Developer Certificate of Origin](https://github.com/amosproj/amos2026ss01-zero-downtime-linux-updates/blob/main/DCO).

See [CONTRIBUTING.md](https://github.com/amosproj/amos2026ss01-zero-downtime-linux-updates/blob/main/CONTRIBUTING.md) for full contribution guidelines.

---

## Building the Project

### Build all workspace crates (debug mode)

```bash
cargo build
```

Compiled binaries are placed in `target/debug/`:

- `target/debug/amos-orchestrator`
- `target/debug/amos-api-server`

### Build in release mode (optimised, for deployment)

```bash
cargo build --release
```

Compiled binaries are placed in `target/release/`:

- `target/release/amos-orchestrator`
- `target/release/amos-api-server`

### Build a specific crate only

```bash
cargo build -p amos-orchestrator
cargo build -p amos-api-server
```

---

## ISO Building

[ISO Build](./bootc-build/iso.md)

---

## Running Tests

### Run all tests

```bash
cargo test
```

### Run tests for a specific crate

```bash
cargo test -p amos-api-server
cargo test -p amos-orchestrator
cargo test -p amos-common
```

### Integration testing

To test the whole system in the sense of e2e integration tests, see the e2e scripts. Further description of the e2e tests can be found at [scripts](./scripts.md)

### Notable test coverage

| Crate | Tests |
|-------|-------|
| `amos-orchestrator` | CLI flag parsing (`--self-check`, `--config`, `--debug`) |
| `amos-orchestrator` | App reconciliation logic (`ReconcileIterator` actions) |
| `amos-common` | `device_api` models JSON serialization/deserialization (`os`, `apps`, `logs`, `register`) |

---

## Running Locally

### 0. Start *a* database

The api server requires a database, which by default is expected as a Postgres instance running on `localhost:5432`. A local container instance can be managed like this:

```bash
cd .devcontainer/

# start the container
podman-compose up -d postgres

# stop the container
podman-compose down postgres

# optional: to delete the data (volume)
podman volume rm devcontainer_postgres_data
```

**Alternatively for local development** an sqlite database can be used. For that, just define a sqlite connection string via the config or an environment variable when running the server, e.g.: `APP_DATABASE_URL="sqlite://db.sqlite?mode=rwc" cargo run ..."`

### 1. Start the API server

The server simulates the Cloud API on `localhost:80`:

```bash
# Requires port 80 — run with sudo or change to a high port (edit source if needed)
sudo ./target/debug/amos-api-server
```

> **Note:** Port 80 requires root privileges. If you do not want to use `sudo`, edit `api-server/src/main.rs` to bind to a high port (e.g. `8080`) and rebuild.

Place any binary update artifacts you want to serve in an `assets/` directory next to the binary.

### 2. Create a config file

```bash
cp orchestrator/config.example.toml config.toml
```

Edit `config.toml` to point at the server:

```toml
cloud_url = "http://localhost/v1"
poll_interval_secs = 10
```

> **Note:** `cloud_url` accepts both `http://` and `https://` schemas. For local development, pointing directly at the plain-HTTP server with `http://localhost` is the simplest setup.

### 3. Run the Orchestrator

```bash
./target/debug/amos-orchestrator --config config.toml -d
```

The `-d` flag enables `INFO` level logging so you can follow the update loop.

### 4. Verify with self-check

```bash
./target/debug/amos-orchestrator --self-check --config config.toml
```

Exit code 0 means the agent is correctly configured and inventory tooling is available.

---

## Deploying to an Edge Device

Process and commands are similar to that of local testing with lima: [dev-env/lima.md](dev-env/lima.md). A production update of orchestrator should be done via a OS version update / OS image with new orchestrator binary.

---

## Container / rootc Build

The `bootc-build/` directory contains the files needed to embed the Orchestrator into an OS container image.

### Files

| File | Description |
|------|-------------|
| `bootc-build/Containerfile` | OCI image definition for the root container build |
| `bootc-build/orchestrator.service` | systemd unit file bundled into the image |

### Container Security & Signature Policies

The project enforces image validation checks via container signature policies to secure the Edge IPC environment against untrusted code execution. These rules dictate how the host container engine (Podman) verifies image integrity before pulling or staging updates.

The mechanism switches between two structural configuration files depending on your build target:

- `container-policy.json` (Production Default): Configures a locked-down profile. It sets the default behavior to reject all unconfigured registries and mandates that all container updates are strictly signed (sigstoreSigned) using the cryptographic public key stored at `/usr/share/pki/containers/cosign.pub`.
- `container-policy.dev.json` (Development Override): Provides a permissive fallback for local loop validation. While it maintains strict signature checking for official remote GHCR layers, it introduces exceptions (insecureAcceptAnything) for `localhost/amos-edge images and native containers-storage flows to enable rapid local debugging.

### Build the container image

```bash
podman build -f bootc-build/Containerfile -t amos-orchestrator:latest .
```

### Push to GHCR

```bash
podman tag amos-orchestrator:latest ghcr.io/amosproj/amos2026ss01-zero-downtime-linux-updates-system:latest
podman push ghcr.io/amosproj/amos2026ss01-zero-downtime-linux-updates-system:latest
```

---

## CI / CD

The project uses GitHub Actions. Workflows are defined under `.github/workflows/` (if present). The typical pipeline:

1. **Build** — `cargo check` on push/PR
2. **Lint** — `cargo clippy -- -D warnings`
3. **Format check** — `cargo fmt --check`
4. **Test** — `cargo test` on all crates
5. **Release build** — `cargo build --release` on tagged push

Changelogs are auto-generated from conventional commits using [`git-cliff`](https://github.com/orhun/git-cliff) (configured in `cliff.toml`).

Also, see [CI Pipeline](./ci.md)

## Environment Variables Reference

> See [User Documentation — Configuration](user_documentation.md#configuration) for the full environment variable reference.

---

## Subsystem Deep Dives

### Application Log Aggregation & Backpressure

The Orchestrator centralizes container log collection via an asynchronous multiplexing registry task.

- **Timestamp Extraction:** Containers are tailed with runtime-enforced timestamps. The ingestion pipeline strips the RFC3339-nano prefix applied by the container engine to preserve the exact execution time, falling back to local time only if the log line lacks a valid prefix.
- **Flushing and Batching:** To minimize network overhead, logs are grouped into batches (`log_max_batch`) and pushed periodically (`log_flush_interval_secs`).
- **Memory Protection (Backpressure):** If connection to the Cloud API fails, logs are buffered in memory up to a hard cap (`log_max_buffer`). When the buffer fills completely, backpressure is enforced by evicting the oldest log lines first, preventing edge device out-of-memory (OOM) faults.

### Subprocess Execution & Lifecycle Exit Codes

Operating system updates (`bootc`) and system-level actions are safely wrapped inside an isolated command execution layer.

- **Stream Deadlocks:** The executer drains `stdout` and `stderr` streams concurrently using asynchronous line loops. This ensures that fast-exiting processes do not leave unread data in system buffers, avoiding truncated logs.
- **Reboot Imminence (Exit Code 137):** Processes terminated without a clean status return or killed by system signals return exit code `137`. During OS upgrade and rollback phases, code `137` is explicitly intercepted and handled as a successful transaction, indicating that the system engine is dropping execution to perform an immediate hardware reboot.

### Hardware Identity & TPM 2.0 Security

The system binds device identity and authorization tokens directly to physical (or emulated) hardware primitives.

#### Hardware Identity Paths

The Orchestrator validates identity via DMI/SMBIOS tables using the following files:

- Primary Unique Identifier: `/sys/class/dmi/id/product_uuid`
- Fallback Identifier: `/sys/class/dmi/id/board_serial` (utilized to accommodate specific university reference hardware constraints).

> Note: String validation rules automatically reject generic OEM placeholders such as "Not Specified" or "To Be Filled By O.E.M.".

#### Cryptographic Architecture & Handle Allocation

Device security relies on a TPM 2.0 interface communicating over `/dev/tpmrm0`. Cryptographic operations adhere to the following layout:

| Asset / Operation | Primitive Details | TPM Handle / Path Constraint |
| --- | --- | --- |
| **Endorsement Key (EK)** | RSA Public Key extraction | `0x8101_0001` (Read directly via reference convention) |
| **Device Signing Key** | 2048-bit RSA (Owner hierarchy) | `0x8100_A038` (Created and persisted if missing) |
| **Data Signing Scheme** | RSASSA-PKCS1-v1_5 + SHA256 | Handled in an isolated Null-Auth session |

#### Proactive JWT Management

Cloud API interactions require a Device JWT signed by the TPM. To maintain an uninterrupted connection state, the system employs a proactive refresh window: token validity is re-evaluated during every cycle, and a new signed JWT is requested exactly `30 seconds` prior to token expiration (`REFRESH_BEFORE`), preventing request drops caused by clock drift.

### Cross-Loop Interlocking (OS Upgrade Freeze)

To prevent container mutations or log shipping corruption while the host system undergoes structural upgrades, the Orchestrator employs a thread-safe synchronization state (`os_upgrade_in_progress: Arc<AtomicBool>`).

- **Behavior:** Whenever an OS update is being actively applied (either immediately or following the expiration of a deferred timer), this atomic flag is flipped to `true`.
- **Impact:** The application reconciliation loop instantly freezes its execution cycle, printing a diagnostic freeze message and bypassing any container creation, modification, or teardown tasks until the system initiates its hardware reboot.
