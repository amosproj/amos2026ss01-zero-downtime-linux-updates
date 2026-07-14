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
8. [Logging & Verbosity](#logging--verbosity)
9. [Running as a systemd Service](#running-as-a-systemd-service)
10. [Rollback & Error Recovery](#rollback--error-recovery)
11. [API Server](#api-server)

---

## Overview

The **Orchestrator** is a background daemon running on Edge IPC nodes. It coordinates real-time state enforcement for operating system images and application workloads using native `bootc` and `podman` abstractions.

```
Orchestrator  ──►  OS Update Loop   (Monitors OS image targets, manages deferred reboots)
              ──►  App Update Loop  (Calculates and drives real container mutations)
              ──►  Aliveness Loop   (Maintains continuous device heartbeats)
```

---

## Prerequisites

| Requirement | Notes |
|-------------|-------|
| Linux | rpm-ostree/bootc compatible OS recommended for future update support |
| `bootc` | Used for updates (`bootc switch`) |
| `podman` | Used for container management |
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

The Orchestrator reads its configuration from a TOML file. The file path is resolved in the following order of precedence:

1. The `--config <FILE>` CLI flag (required if given — startup fails if the file is missing).
2. The `APP_CONFIG_FILE` environment variable (required if set).
3. `config.toml` in the current working directory (optional — defaults are used if absent).

A ready-to-use template with inline documentation for all available options is provided at [`orchestrator/config.example.toml`](https://github.com/amosproj/amos2026ss01-zero-downtime-linux-updates/blob/main/orchestrator/config.example.toml). Copy it and adjust the values for your environment:

```bash
cp orchestrator/config.example.toml config.toml
```

### Environment variable overrides

All config values can be overridden with environment variables prefixed `APP_`:

### Configuration Options

| Environment Variable | Config Key | Default / Type | Description |
|----------------------|------------|----------------|-------------|
| `APP_CLOUD_URL` | `cloud_url` | String (Required) | Base URL for the cloud management endpoints |
| `APP_PODMAN_PATH` | `podman_path` | String | Path locating the Podman socket connection interface |
| `APP_POLL_INTERVAL_SECS` | `poll_interval_secs` | u64 | Target evaluation loop frequency |
| `APP_LOG_FLUSH_INTERVAL_SECS` | `log_flush_interval_secs`| u64 | Delay between log database shipping cycles |
| `APP_LOG_MAX_BATCH` | `log_max_batch` | usize | Maximum log items grouped in a single payload |
| `APP_LOG_MAX_BUFFER` | `log_max_buffer` | usize | Maximum log items retained during network outages |
| `APP_DEFERRED_SWITCH_TIMER_SECS`| `deferred_switch_timer_secs`| u64 | Grace time allowed before non-immediate OS updates trigger a reboot |

> **Note:** `APP_CONFIG_FILE` is special — it selects *which* config file to load (see precedence above) rather than overriding a value. The `--config` flag takes precedence over it.

### Validation rules

- `cloud_url` **must** begin with `http://` or `https://`.
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

The `--self-check` flag validates the system configuration and bootc/podman tooling without starting the main loop. Use it to verify the agent is correctly set up:

```bash
amos-orchestrator --self-check
amos-orchestrator --self-check --config /etc/amos/config.toml
```

Exit codes:
- `0` — all checks passed
- `1` — one or more checks failed (details printed to stderr)

---

## Logging & Verbosity

Log output is written to **stderr**. Default level is `WARN`. Use `-d` / `-dd` flags to increase verbosity:

| Flags | Level |
|-------|-------|
| *(none)* | `WARN` |
| `-d` | `INFO` |
| `-dd` | `DEBUG` |
| `-ddd` | `TRACE` |

> **Advanced:** The standard `RUST_LOG` environment variable can be used for granular per-module filtering (e.g. `RUST_LOG=amos_orchestrator=debug`). When `-d` flags are provided on the command line they take precedence over `RUST_LOG`.

---

## Running as a systemd Service

A systemd unit file is provided in [`bootc-build/orchestrator.service`](https://github.com/amosproj/amos2026ss01-zero-downtime-linux-updates/blob/main/bootc-build/orchestrator.service). To install it:

```bash
sudo cp bootc-build/orchestrator.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now amos-orchestrator
```

Check status and logs:

```bash
sudo systemctl status amos-orchestrator
sudo journalctl -u amos-orchestrator -f
```

---

## Rollback & Error Recovery

If an update causes a problem you can manually trigger a rollback using the standard OS tooling:

```bash
# OS rollback via bootc
sudo bootc rollback
```

For application containers, use `podman` to switch back to the previous image tag manually. Automated rollback support will be added in a future sprint.

### API Reference

For a complete technical breakdown of the network communication protocols, data models, and active endpoints, please refer to the OpenAPI specification files:

* [Device User API Reference](./DeviceApi/openapi_user.yaml)
* [Full Device API Specification](./DeviceApi/openapi.yaml)