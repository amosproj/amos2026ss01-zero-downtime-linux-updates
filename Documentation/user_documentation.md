# User Documentation

## Zero-Downtime Linux Updates

This document explains how to install, configure, and operate the **Orchestrator** agent on an Edge IPC device.

---

## Table of Contents

1. [Overview](#overview)
2. [Prerequisites](#prerequisites)
3. [Installation](#installation)
4. [Configuration](#configuration)
5. [Running the Orchestrator](#running-the-orchestrator)
6. [CLI Reference](#cli-reference)
7. [Self-Check / Health Check](#self-check--health-check)
8. [Inventory](#inventory)
9. [Logging & Verbosity](#logging--verbosity)
10. [Running as a systemd Service](#running-as-a-systemd-service)
11. [API Mock Server](#api-mock-server)

---

## Overview

The **Orchestrator** is a background agent that runs on each Edge IPC. It periodically compares the desired state of the OS and application containers against the host's current state and is designed to trigger updates when they differ.

> **Current status:** The update loops and reconciliation logic are in place, but the actual `bootc`/`rpm-ostree` OS update commands and Podman container management calls are **placeholder stubs** — they log output but do not yet perform real updates.

```
Orchestrator  ──►  OS Update Loop   (compares OS state, calls placeholder update)
              ──►  App Update Loop  (reconciles container state, calls placeholder fns)
              ──►  Inventory        (collects device info, writes to local JSON file)
```

---

## Prerequisites

| Requirement | Notes |
|-------------|-------|
| Linux | Any distro; rpm-ostree/bootc compatible OS recommended for future update support |
| `bootc` | Optional — used for inventory collection (`bootc status`); update commands not yet wired |
| `podman` | Optional — used for inventory collection (`podman ps`); container management not yet wired |
| Network access to Cloud API | HTTPS required (see Configuration) |
| Rust toolchain | Only needed if building from source |

---

## Installation

### Pre-built binary

Copy the `amos-orchestrator` binary to the target device and place it in `/usr/local/bin/`:

```bash
sudo cp amos-orchestrator /usr/local/bin/amos-orchestrator
sudo chmod +x /usr/local/bin/amos-orchestrator
```

### Building from source

See the [Build & Deploy Documentation](build_documentation.md) for full build instructions.

---

## Configuration

The Orchestrator reads its configuration from a TOML file. By default it looks for `config.toml` in the working directory. A custom path can be passed via `--config`.

### Example config file

```toml
# URL of the Cloud API (must start with https://)
cloud_url = "https://your-cloud-api.example.com/api/v1"

# How often to poll the Cloud API, in seconds (must be >= 1)
poll_interval_secs = 60

# Where to write the device inventory JSON file
inventory_path = "./inventory/inventory.json"
```

A ready-to-use template is provided at [`orchestrator/config.example.toml`](../orchestrator/config.example.toml).

### Environment variable overrides

All config values can be overridden with environment variables prefixed `APP_`:

| Environment variable | Config key |
|----------------------|------------|
| `APP_CLOUD_URL` | `cloud_url` |
| `APP_POLL_INTERVAL_SECS` | `poll_interval_secs` |
| `APP_INVENTORY_PATH` | `inventory_path` |

### Validation rules

- `cloud_url` **must** begin with `https://`.
- `poll_interval_secs` **must** be ≥ 1.

---

## Running the Orchestrator

```bash
# Use default config.toml in current directory
amos-orchestrator

# Specify a custom config file
amos-orchestrator --config /etc/amos/config.toml

# Enable verbose logging (repeat for more verbosity)
amos-orchestrator -d        # debug
amos-orchestrator -dd       # trace
```

Stop the agent with **Ctrl+C** (SIGINT).

---

## CLI Reference

```
Usage: amos-orchestrator [OPTIONS]

Options:
  -s, --self-check         Run self-check instead of the main loop
  -c, --config <FILE>      Path to a custom config file
  -d, --debug...           Increase log verbosity (repeatable: -d, -dd)
  -h, --help               Print help
  -V, --version            Print version
```

---

## Self-Check / Health Check

The `--self-check` flag validates the system configuration and inventory tooling without starting the main loop. Use it to verify the agent is correctly set up:

```bash
amos-orchestrator --self-check
amos-orchestrator --self-check --config /etc/amos/config.toml
```

Exit codes:
- `0` — all checks passed
- `1` — one or more checks failed (details printed to stderr)

---

## Inventory

On startup, the Orchestrator collects a **device inventory** and writes it as a JSON file to the path defined by `inventory_path`. The inventory includes:

| Section | Contents |
|---------|----------|
| `system` | Hostname, OS name/version, kernel version |
| `deployments` | rpm-ostree deployment info (checksum, version, booted/staged flags) |
| `bootc_status` | Booted, staged, and rollback image info |
| `applications` | Running application container names and versions |

If a section cannot be collected (e.g. `bootc` is not installed), the field is marked `"status": "unavailable"` with a reason — the rest of the inventory is still written.

---

## Logging & Verbosity

Log output is written to **stderr**. Default level is `WARN`. Use `-d` / `-dd` flags to increase verbosity:

| Flags | Level |
|-------|-------|
| *(none)* | `WARN` |
| `-d` | `INFO` |
| `-dd` | `DEBUG` |
| `-ddd` | `TRACE` |

---

## Running as a systemd Service

A systemd unit file is provided in [`rootc-build/orchestrator.service`](../rootc-build/orchestrator.service). To install it:

```bash
sudo cp rootc-build/orchestrator.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now amos-orchestrator
```

Check status and logs:

```bash
sudo systemctl status amos-orchestrator
sudo journalctl -u amos-orchestrator -f
```

---

## API Mock Server

During development or testing, a local mock server (`amos-api-mock-server`) can stand in for a real Cloud API. It serves a static catalog at `GET /v1/catalog` and static download assets from a local `assets/` directory.

> **Note:** The mock server runs on plain HTTP (port 80). The Orchestrator config validates that `cloud_url` starts with `https://`, so pointing it at `http://localhost` will fail validation. For local testing, use a TLS-terminating reverse proxy (e.g. `nginx` or `caddy`) in front of the mock server, or temporarily relax the validation in a development branch.

```bash
# Start mock server on port 80
sudo ./amos-api-mock-server
```

The catalog response from the mock server looks like:

```json
[
  { "name": "os",  "version": "1.2.3", "url": "ghcr.io/amosproj/...", "signature": "AAAA..." },
  { "name": "app", "version": "4.5.6", "url": "/v1/download/app4.5.6", "signature": "AAAA..." }
]
```
