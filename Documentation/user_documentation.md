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
11. [Rollback & Error Recovery](#rollback--error-recovery)
12. [API Mock Server](#api-mock-server)

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

The Orchestrator reads its configuration from a TOML file. The file path is resolved in the following order of precedence:

1. The `--config <FILE>` CLI flag (required if given — startup fails if the file is missing).
2. The `APP_CONFIG_FILE` environment variable (required if set).
3. `config.toml` in the current working directory (optional — defaults are used if absent).

A ready-to-use template with inline documentation for all available options is provided at [`orchestrator/config.example.toml`](../orchestrator/config.example.toml). Copy it and adjust the values for your environment:

```bash
cp orchestrator/config.example.toml config.toml
```

### Environment variable overrides

All config values can be overridden with environment variables prefixed `APP_`:

| Environment variable | Config key | Description |
|----------------------|------------|-------------|
| `APP_CLOUD_URL` | `cloud_url` | Cloud API base URL |
| `APP_POLL_INTERVAL_SECS` | `poll_interval_secs` | Poll frequency in seconds |
| `APP_INVENTORY_PATH` | `inventory_path` | Inventory output file path |
| `https_proxy` | — | HTTPS proxy URL (reqwest default) |

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

> **Advanced:** The standard `RUST_LOG` environment variable can be used for granular per-module filtering (e.g. `RUST_LOG=amos_orchestrator=debug`). When `-d` flags are provided on the command line they take precedence over `RUST_LOG`.

---

## Running as a systemd Service

A systemd unit file is provided in [`bootc-build/orchestrator.service`](../bootc-build/orchestrator.service). To install it:

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

> **Future release:** Automated rollback and error recovery are not yet implemented.

If an update causes a problem you can manually trigger a rollback using the standard OS tooling:

```bash
# OS rollback via bootc
sudo bootc rollback

# OS rollback via rpm-ostree
sudo rpm-ostree rollback
```

For application containers, use `podman` to switch back to the previous image tag manually. Automated rollback support will be added in a future sprint.

---

## API Mock Server

During development or testing, a local mock server (`amos-api-mock-server`) can stand in for a real Cloud API. It serves a static catalog at `GET /v1/catalog` and static download assets from a local `assets/` directory.

> **Note:** The mock server runs on plain HTTP (port 80). The Orchestrator config accepts both `http://` and `https://` URLs, so you can point it directly at `http://localhost` for local testing without a reverse proxy.

```bash
# Start mock server on port 80 (requires root)
sudo ./amos-api-mock-server
```

> **Tip:** To avoid `sudo`, edit `api-mock-server/src/main.rs` to bind to a high port (e.g. `8080`) and rebuild. Then set `cloud_url = "http://localhost:8080/api/v1"` in your config.

The catalog response from the mock server looks like:

```json
[
  { "name": "os",  "version": "1.2.3", "url": "ghcr.io/amosproj/...", "signature": "AAAA..." },
  { "name": "app", "version": "4.5.6", "url": "/v1/download/app4.5.6", "signature": "AAAA..." }
]
```

### API Reference

All routes are served under `/v1`. Fields marked `*` are required. Pagination is available for all list routes with `?page=x&page_size=y`, defaulting to page 1 and page size 20.

**Device Summaries** _(read-only)_

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/v1/devices/summary` | List device summaries. Query options: `?group_id=<id>&tenant_id=<id>&uuid=<string>&hostname=<string>` |
| `GET` | `/v1/devices/{id}/summary` | Get a single device summary |

**Audit Logs** _(read-only)_

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/v1/audit-logs` | List audit logs. Query options: `?table_name=<string>&record_id=<string>&changed_by=<int>&operation=<string>` |
| `GET` | `/v1/audit-logs/{table_name}/{record_id}` | Get audit logs for a specific table and record id. |
| `GET` | `/v1/audit-logs/by-device/{id}` | Get audit logs for a specific device. |

**Tenants**

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/v1/tenants` | List tenants. Query options: `?name=<string>` |
| `POST` | `/v1/tenants` | Create — body: `{ name*, description }` |
| `GET` | `/v1/tenants/{id}` | Get by ID |
| `PUT` | `/v1/tenants/{id}` | Replace by ID |
| `DELETE` | `/v1/tenants/{id}` | Delete — 204 on success |

**Groups**

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/v1/groups` | List groups. Query options: `?name=<string>` |
| `POST` | `/v1/groups` | Create — body: `{ name* }` |
| `GET` | `/v1/groups/{id}` | Get by ID |
| `PUT` | `/v1/groups/{id}` | Replace by ID |
| `DELETE` | `/v1/groups/{id}` | Delete — 204 on success |

**Devices**

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/v1/devices` | List devices. Query options: `?group_id=<id>&tenant_id=<id>&uuid=<string>&hostname=<string>` |
| `POST` | `/v1/devices` | Create — body: `{ uuid*, hostname*, tenant_id, group_id }` |
| `GET` | `/v1/devices/{id}` | Get by ID |
| `PUT` | `/v1/devices/{id}` | Replace by ID |
| `DELETE` | `/v1/devices/{id}` | Delete — 204 on success |

**Applications**

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/v1/applications` | List applications. Query options: `?name=<string>` |
| `POST` | `/v1/applications` | Create — body: `{ name*, description }` |
| `GET` | `/v1/applications/{id}` | Get by ID |
| `PUT` | `/v1/applications/{id}` | Replace by ID |
| `DELETE` | `/v1/applications/{id}` | Delete — 204 on success |

**Application Configs**

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/v1/app-configs` | List configs. Query options: `?application_id=<id>` |
| `POST` | `/v1/app-configs` | Create — body: `{ application_id, image*, config, comment }` |
| `GET` | `/v1/app-configs/{id}` | Get by ID |
| `PUT` | `/v1/app-configs/{id}` | Replace by ID |
| `DELETE` | `/v1/app-configs/{id}` | Delete — 204 on success |

**Application Assignments**

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/v1/app-assignments` | List. Query options: `?application_config_id=<id>&device_id=<id>&group_id=<id>` |
| `POST` | `/v1/app-assignments` | Create — body: `{ application_config_id, device_id, group_id }` — `device_id` or `group_id` required |
| `GET` | `/v1/app-assignments/{id}` | Get by ID |
| `PUT` | `/v1/app-assignments/{id}` | Replace by ID |
| `DELETE` | `/v1/app-assignments/{id}` | Delete — 204 on success |

**Reported Application Assignments** _(device-originated — no POST/PUT)_

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/v1/reported-app-assignments` | List. Query options: `?application_config_id=<id>&device_id=<id>` |
| `GET` | `/v1/reported-app-assignments/{id}` | Get by ID |
| `DELETE` | `/v1/reported-app-assignments/{id}` | Delete — 204 on success |

**OS Versions**

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/v1/os-versions` | List OS versions. |
| `POST` | `/v1/os-versions` | Create — body: `{ commit_hash*, orchestrator_version*, description }` |
| `GET` | `/v1/os-versions/{id}` | Get by ID |
| `PUT` | `/v1/os-versions/{id}` | Replace by ID |
| `DELETE` | `/v1/os-versions/{id}` | Delete — 204 on success |

**OS Assignments**

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/v1/os-assignments` | List. Query options: `?os_version_id=<id>&device_id=<id>&device_uuid=<string>&group_id=<id>` |
| `POST` | `/v1/os-assignments` | Create — body: `{ os_version_id, device_id, group_id }` — `device_id` or `group_id` required |
| `GET` | `/v1/os-assignments/{id}` | Get by ID |
| `PUT` | `/v1/os-assignments/{id}` | Replace by ID |
| `DELETE` | `/v1/os-assignments/{id}` | Delete — 204 on success |

**Reported OS Assignments** _device originated — no PUT_

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/v1/reported-os-assignments` | List. Query options: `?device_id=<id>&os_version_id=<id>` |
| `POST` | `/v1/reported-os-assignments` | Create — body: `{ os_version_id*, device_id }` Device id can be replaced by Query option `?device_uuid=<string>` |
| `GET` | `/v1/reported-os-assignments/{id}` | Get by ID |
| `DELETE` | `/v1/reported-os-assignments/{id}` | Delete — 204 on success |

### Error responses

All errors return JSON: `{ "error": "<message>" }`

| Status | Meaning |
|--------|---------|
| `404 Not Found` | No resource with that ID |
| `422 Unprocessable Entity` | Validation failed (empty required field; missing `device_id`/`group_id`) |
| `500 Internal Server Error` | Database error |
