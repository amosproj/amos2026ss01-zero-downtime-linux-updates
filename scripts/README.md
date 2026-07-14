# Scripts & Testing Overview

### `common_env.sh`

**Purpose**: A shared environment utility file that centralizes configuration parameters, hardcoded development JWT authentication tokens, VM identifiers, and core helper functions (`api()`, `start_swtpm()`, `ensure_vm_running()`). The `api()` helper wraps `curl` requests to validate that backend network operations return expected HTTP status codes, throwing an assertion failure if they deviate.

### `connect_db.sh`

**Purpose**: A convenience developer shortcut that executes a `psql` command to log directly into the local TimescaleDB container instance using default development credentials (`U: app`, `P: 4M0S`, `D: amos_timeseries` on port `55433`).

### `create_tpm.sh`

**Purpose**: This script initializes an emulated software TPM (`swtpm`) to mirror the exact environment configuration of the project's reference hardware. It configures transient isolated XDG home directories and calls `swtpm_setup` to establish endorsement keys and lock NVRAM.

### `create_tpm.sh`

**Purpose**: Configures an isolated workspace environment to provision an emulated vTPM instance mirroring reference hardware characteristics. It overrides `XDG` path variables to prevent local host contamination, leverages host packages like `swtpm_setup` to generate endorsement keys (`createek`), configures validation certificates, and locks the NVRAM state cleanly.

### `docs-landing.html`

**Purpose**: Serves as the central static web portal and entry point for the AMOS project documentation and Rust API reference. It provides structured layouts linking to architecture, deployment guides, fundamentals (like `bootc`), and the Rust documentation pages.

### `docs.sh`

**Purpose**: This script automates compiling the project's unified documentation hub. It coordinates standard `cargo doc` compilation with an `mdBook` narrative layer, copies external device/hypervisor markdown guides (`tpm.md`, `lima.md`), and packages the OpenAPI dashboards (`swagger-ui.html`, `swagger-ui-user.html`) under a unified `target/doc/` path. It supports a `SKIP_RUSTDOC=1` flag for rapid local text editing on host platforms where the Linux-only TPM components cannot compile.

### `e2e_run_all.sh`

**Purpose**: This script functions as the primary end-to-end test runner, orchestrating a complex test infrastructure lifecycle by wiping stale states, spinning up a local `swtpm`, a QEMU-backed VM, a TimescaleDB container, and booting the API server in the background. It runs a sequential suite of integration checks and safely handles cleanup and final summary generation on completion.

### `e2e_setup_harness.sh`

**Purpose**: Orchestrates the setup lifecycle for complete end-to-end integration tests. It flushes out stale configurations, starts a Unix socket-bound software TPM, spins up a temporary TimescaleDB Podman container, and boots a Lima VM (`edge-ipc`) injected with custom QEMU hardware identifiers (SMBIOS UUID/Serial). Finally, it compiles the local host `amos-orchestrator` agent, injects it into the running VM, overrides the systemd service to target this fresh binary, and launches the mock cloud API server.

### `flash-usb.sh`

**Purpose**: A destructive system utility meant to flash an installer ISO onto a target USB block device using direct block writing via `dd`. It contains intentional safety guardrails that detect and refuse to overwrite active system root disks or mounted partitions.

### `generate_jwks.py`

**Purpose**: A Python script designed to generate cryptographic key pairs using either Ed25519 or RSA-4096 algorithms. It exports the resulting public and private keys in standard PEM format to sign and verify JWT authentication tokens during test cycles.

### `onboard_device.sh` (or `seed_device.sh`)

**Purpose**: Orchestrates and validates the cryptographic zero-touch provisioning flow. It extracts the virtual TPM 2.0 Endorsement Key (EK) public token directly from the hardware hierarchy (`0x81010001`), extracts its RSA component, pairs it with the QEMU SMBIOS serial number, provisions a fallback tenant (`Weber-Lager`), and posts a payload to the cloud API's pending registration queue. It then restarts the daemon to verify successful automated self-onboarding.

### `prepare-commit-msg`

**Purpose**: A Git client-side hook installed via `make setup` that automates Developer Certificate of Origin (DCO) compliance. It reads the contributor's local `git config` name and email to automatically append a `Signed-off-by:` line to the bottom of the commit message, bypassing merge or squash commit payloads.

### `seed_db.sh`

**Purpose**: A setup helper that dynamically synchronizes the cloud API server's initial database state with the actual state of the running virtual environment. Instead of relying on hardcoded image references, it queries the live Lima target VM's current active OSTree checksum via `bootc status --json` and uses that hash to anchor the very first valid `os-versions` and `os-assignments` table records.

### `swagger-ui.html`

**Purpose**: An HTML dashboard wrapper that loads the external Swagger UI library to parse and display the device-facing OpenAPI specification file (`device_api.yaml`).

### `swagger-ui-user.html`

**Purpose**: An HTML file that implements Swagger UI to expose, inspect, and test the administrative user-facing OpenAPI endpoints (`openapi_user.yaml`).

### `test_apps.sh`

**Purpose**: An automated integration test verifying application placement capabilities. It interacts with the API server to create a tenant application reference using a sample "hello-world" image, restarts the orchestrator inside the VM, and polls for device state reporting. It validates that runtime environment variables (`NAME=AMOS`) pass through to application logs, then triggers a delete call to ensure the orchestrator successfully destroys the container via Podman inside the VM.

### `test_bootc_deferred.sh`

**Purpose**: Specifically validates timer-deferred, atomic operating system updates. It modifies the node configuration file (`/etc/amos/config.toml`) to set a 30-second deferred timer, provisions a new non-immediate OS version assignment (`immediate: false`) on the API server, and restarts the agent loop. It then polls the node's transactional engine (`bootc status --json`) to confirm that the target containerized OS successfully stages and applies after the timeout window closes.

### `test_bootc_status.sh`

**Purpose**: Tests node-to-cloud status tracking paths. It queries the local Lima VM to extract its current deployment OSTree checksum via `bootc status --json`, registers that exact checksum value as the active baseline in the API server, and forces an orchestrator check-in. It then asserts that the API server correctly parses the incoming telemetry and reports that the device successfully aligns with target baseline ID 1.

### `test_bootc_switch.sh`

**Purpose**: Validates rapid, immediate operating system atomic upgrades. It pushes an upgrade assignment to the cloud endpoint with the `immediate: true` flag set, drops obsolete tracking constraints, and prompts an orchestrator agent execution pass. It dynamically loops over the VM's status to confirm that the node downloads, applies, and successfully reboots into the new containerized image tag without human intervention.

### `test_logs.sh`

**Purpose**: An integration test script that sets up an isolated environment with a temporary TimescaleDB container to comprehensively validate log submission and retrieval. It issues mock payloads verifying HTTP codes for device logs, application logs, and the Server-Sent Events (SSE) streaming filters.

### `test_self_checks.sh`

**Purpose**: A comprehensive negative-testing validation suite that exercises the orchestrator's structural integrity guardrails using its self-check flag (`-s`). It systematically degrades the running environment to ensure the agent traps failures cleanly:

  1. **Normal State**: Verifies a successful pass.
  2. **Container Infrastructure Failure**: Halts the Podman socket and expects a socket connectivity error trap.
  3. **TPM Absence**: Boots a QEMU instance stripped of its vTPM architecture and catches initialization failures.
  4. **Cryptographic Identity Corruption**: Spins up an unprovisioned/locked raw vTPM state to test key extraction failures.
  5. **Hardware Spoofing Protection**: Boots without valid SMBIOS DMI identifiers to confirm asset verification blocks execution.

### `test_server.sh`

**Purpose**: An integration script validating standard REST API functionality such as tenant setup, device registrations, and desired OS target assignments. It provisions an in-memory SQLite database configuration to run rapid CRUD operations and assertion loops.

### `test_unsigned_denied.sh`

**Purpose**: Validates the zero-trust security framework by verifying that an unsigned containerized OS image reference is explicitly rejected by the node. It captures the running system's `bootc` OSTree checksum, injects an unsigned target image into the API server, forces an orchestrator iteration, checks `journalctl` logs for cryptographic signature rejection signatures (`signature|rejected|denied`), and asserts that the system state remained completely unchanged.
