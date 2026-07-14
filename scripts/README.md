# Scripts & Testing Overview

### `common_env.sh`

**Purpose**: A shared environment utility file that centralizes configuration parameters, hardcoded development JWT authentication tokens, VM identifiers, and core helper functions (`api()`, `start_swtpm()`, `ensure_vm_running()`). The `api()` helper wraps `curl` requests to validate that backend network operations return expected HTTP status codes, throwing an assertion failure if they deviate.

**Documentation Status**: This file is an internal script dependency; it must be sourced by all integration sub-tests to establish identical testing baselines.

### `connect_db.sh`

**Purpose**: A convenience developer shortcut that executes a `psql` command to log directly into the local TimescaleDB container instance using default development credentials (`U: app`, `P: 4M0S`, `D: amos_timeseries` on port `55433`).

**Documentation Status**: Primarily an unexposed quality-of-life workflow helper for backend development, useful during manual database schema migrations or log verification checks.

### `create_tpm.sh`

**Purpose**: This script initializes an emulated software TPM (`swtpm`) to mirror the exact environment configuration of the project's reference hardware. It configures transient isolated XDG home directories and calls `swtpm_setup` to establish endorsement keys and lock NVRAM.

**Documentation Status**: This internal behavior is not explicitly highlighted in user-facing setup files, but it serves as a critical block for automated testing environments.

### `create_tpm.sh`

**Purpose**: Configures an isolated workspace environment to provision an emulated vTPM instance mirroring reference hardware characteristics. It overrides `XDG` path variables to prevent local host contamination, leverages host packages like `swtpm_setup` to generate endorsement keys (`createek`), configures validation certificates, and locks the NVRAM state cleanly.

**Documentation Status**: This is the underlying script utilized during local initialization routines; it supports both standalone virtual environment initialization and automated test execution paths.

### `docs-landing.html`

**Purpose**: Serves as the central static web portal and entry point for the AMOS project documentation and Rust API reference. It provides structured layouts linking to architecture, deployment guides, fundamentals (like `bootc`), and the Rust documentation pages.

**Documentation Status**: This file is self-documenting as it serves as the master dashboard organizing the entire documentation stack.

### `docs.sh`

**Purpose**: This script automates compiling the project's unified documentation hub. It coordinates standard `cargo doc` compilation with an `mdBook` narrative layer, copies external device/hypervisor markdown guides (`tpm.md`, `lima.md`), and packages the OpenAPI dashboards (`swagger-ui.html`, `swagger-ui-user.html`) under a unified `target/doc/` path. It supports a `SKIP_RUSTDOC=1` flag for rapid local text editing on host platforms where the Linux-only TPM components cannot compile.

**Documentation Status**: This script acts as the operational logic driving `make docs` and the automated GitHub Actions CI site deployment workflow. It ensures that the generated output strictly matches the structure displayed in `docs-landing.html`.

### `e2e_run_all.sh`

**Purpose**: This script functions as the primary end-to-end test runner, orchestrating a complex test infrastructure lifecycle by wiping stale states, spinning up a local `swtpm`, a QEMU-backed VM, a TimescaleDB container, and booting the API server in the background. It runs a sequential suite of integration checks and safely handles cleanup and final summary generation on completion.

**Documentation Status**: The high-level test progression covers the core components and is implicitly linked within the developer workspace environment.

### `e2e_setup_harness.sh`

**Purpose**: Orchestrates the setup lifecycle for complete end-to-end integration tests. It flushes out stale configurations, starts a Unix socket-bound software TPM, spins up a temporary TimescaleDB Podman container, and boots a Lima VM (`edge-ipc`) injected with custom QEMU hardware identifiers (SMBIOS UUID/Serial). Finally, it compiles the local host `amos-orchestrator` agent, injects it into the running VM, overrides the systemd service to target this fresh binary, and launches the mock cloud API server.

**Documentation Status**: Represents the foundational execution sequence driving the workspace's complex integration suites. It serves as an internal implementation blueprint for virtual testing setups.

### `flash-usb.sh`

**Purpose**: A destructive system utility meant to flash an installer ISO onto a target USB block device using direct block writing via `dd`. It contains intentional safety guardrails that detect and refuse to overwrite active system root disks or mounted partitions.

**Documentation Status**: This script directly underpins the physical provisioning flows expected in the project's edge deployment and operational documentation.

### `generate_jwks.py`

**Purpose**: A Python script designed to generate cryptographic key pairs using either Ed25519 or RSA-4096 algorithms. It exports the resulting public and private keys in standard PEM format to sign and verify JWT authentication tokens during test cycles.

**Documentation Status**: Serves as a helper tool supporting token-based identity assertions, matching the security parameters mentioned across the logging/device endpoints.

### `onboard_device.sh` (or `seed_device.sh`)

**Purpose**: Orchestrates and validates the cryptographic zero-touch provisioning flow. It extracts the virtual TPM 2.0 Endorsement Key (EK) public token directly from the hardware hierarchy (`0x81010001`), extracts its RSA component, pairs it with the QEMU SMBIOS serial number, provisions a fallback tenant (`Weber-Lager`), and posts a payload to the cloud API's pending registration queue. It then restarts the daemon to verify successful automated self-onboarding.

**Documentation Status**: Serves as the functional verification baseline for the **Device Onboarding & Identity Architecture*section of your system design documentation.

### `prepare-commit-msg`

**Purpose**: A Git client-side hook installed via `make setup` that automates Developer Certificate of Origin (DCO) compliance. It reads the contributor's local `git config` name and email to automatically append a `Signed-off-by:` line to the bottom of the commit message, bypassing merge or squash commit payloads.

**Documentation Status**: Enforces contribution compliance policy transparently at the workspace level, serving as a developer-facing guardrail mirroring remote CI sanity checks.

### `seed_db.sh`

**Purpose**: A setup helper that dynamically synchronizes the cloud API server's initial database state with the actual state of the running virtual environment. Instead of relying on hardcoded image references, it queries the live Lima target VM's current active OSTree checksum via `bootc status --json` and uses that hash to anchor the very first valid `os-versions` and `os-assignments` table records.

**Documentation Status**: An internal deployment lifecycle utility. It provides the core data blueprint required to initialize reproducible local testing environments.

### `swagger-ui.html`

**Purpose**: An HTML dashboard wrapper that loads the external Swagger UI library to parse and display the device-facing OpenAPI specification file (`device_api.yaml`).

**Documentation Status**: Explicitly linked and exposed on the master `docs-landing.html` under the project documentation section.

### `swagger-ui-user.html`

**Purpose**: An HTML file that implements Swagger UI to expose, inspect, and test the administrative user-facing OpenAPI endpoints (`openapi_user.yaml`).

**Documentation Status**: Coexists with the device-facing interface, providing an administrative counterpart for developer and operator auditing.

### `test_apps.sh`

**Purpose**: An automated integration test verifying application placement capabilities. It interacts with the API server to create a tenant application reference using a sample "hello-world" image, restarts the orchestrator inside the VM, and polls for device state reporting. It validates that runtime environment variables (`NAME=AMOS`) pass through to application logs, then triggers a delete call to ensure the orchestrator successfully destroys the container via Podman inside the VM.

**Documentation Status**: Serves as a live, functional spec validating the node application deployment lifecycle described in the project's architecture files.

### `test_bootc_deferred.sh`

**Purpose**: Specifically validates timer-deferred, atomic operating system updates. It modifies the node configuration file (`/etc/amos/config.toml`) to set a 30-second deferred timer, provisions a new non-immediate OS version assignment (`immediate: false`) on the API server, and restarts the agent loop. It then polls the node's transactional engine (`bootc status --json`) to confirm that the target containerized OS successfully stages and applies after the timeout window closes.

**Documentation Status**: Validates the non-disruptive, deferred maintenance windows features crucial to zero-downtime operations documentation.

### `test_bootc_status.sh`

**Purpose**: Tests node-to-cloud status tracking paths. It queries the local Lima VM to extract its current deployment OSTree checksum via `bootc status --json`, registers that exact checksum value as the active baseline in the API server, and forces an orchestrator check-in. It then asserts that the API server correctly parses the incoming telemetry and reports that the device successfully aligns with target baseline ID 1.

**Documentation Status**: Verifies the status-reporting data loops detailed across the project's device-facing API reference specifications.

### `test_bootc_switch.sh`

**Purpose**: Validates rapid, immediate operating system atomic upgrades. It pushes an upgrade assignment to the cloud endpoint with the `immediate: true` flag set, drops obsolete tracking constraints, and prompts an orchestrator agent execution pass. It dynamically loops over the VM's status to confirm that the node downloads, applies, and successfully reboots into the new containerized image tag without human intervention.

**Documentation Status**: Serves as the primary functional test confirming the core zero-downtime, transactional system updates feature.

### `test_logs.sh`

**Purpose**: An integration test script that sets up an isolated environment with a temporary TimescaleDB container to comprehensively validate log submission and retrieval. It issues mock payloads verifying HTTP codes for device logs, application logs, and the Server-Sent Events (SSE) streaming filters.

**Documentation Status**: Acts as live, executable specification text that validates the exact constraints detailed in the endpoints documentation segment. Out-of-date in regards to api endpoints

### `test_self_checks.sh`

**Purpose**: A comprehensive negative-testing validation suite that exercises the orchestrator's structural integrity guardrails using its self-check flag (`-s`). It systematically degrades the running environment to ensure the agent traps failures cleanly:

  1. **Normal State**: Verifies a successful pass.
  2. **Container Infrastructure Failure**: Halts the Podman socket and expects a socket connectivity error trap.
  3. **TPM Absence**: Boots a QEMU instance stripped of its vTPM architecture and catches initialization failures.
  4. **Cryptographic Identity Corruption**: Spins up an unprovisioned/locked raw vTPM state to test key extraction failures.
  5. **Hardware Spoofing Protection**: Boots without valid SMBIOS DMI identifiers to confirm asset verification blocks execution.
**Documentation Status**: Directly maps to your **Node Hardening & Error Handling Resiliency*runtime runbooks, explicitly demonstrating how the system prevents boot loops or inconsistent states if core hardware/software prerequisites go missing.

### `test_server.sh`

**Purpose**: An integration script validating standard REST API functionality such as tenant setup, device registrations, and desired OS target assignments. It provisions an in-memory SQLite database configuration to run rapid CRUD operations and assertion loops.

**Documentation Status**: Complements core database verification tests and verifies the backend lifecycle behavior of edge node tracking.

### `test_unsigned_denied.sh`

**Purpose**: Validates the zero-trust security framework by verifying that an unsigned containerized OS image reference is explicitly rejected by the node. It captures the running system's `bootc` OSTree checksum, injects an unsigned target image into the API server, forces an orchestrator iteration, checks `journalctl` logs for cryptographic signature rejection signatures (`signature|rejected|denied`), and asserts that the system state remained completely unchanged.

**Documentation Status**: This script is the concrete implementation proof for your project's **Image Verification & Security Policy*documentation. It proves that the device cannot be tricked into pulling malicious or unsigned operating system states.
