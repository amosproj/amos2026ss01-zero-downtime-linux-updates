# Local test setup — edge device in a Lima VM

This boots a realistic **edge device** on your machinge so you can see the update
agent run end to end, without any physical hardware

## What it is

- A [Lima](https://lima-vm.io) VM runs a **bootc / Fedora IoT** image — an
  immutable, image-based Linux that updates atomically and can roll back.
- Inside it runs our **orchestrator** (`amos-orchestrator`), the agent that
  checks the cloud for OS/app updates, applies them, and reports a device
  **inventory** back. On boot it writes the inventory to a JSON file and then
  polls the cloud API on an interval.
- The VM image is defined in [`bootc-build/Containerfile`](https://github.com/amosproj/amos2026ss01-zero-downtime-linux-updates/blob/main/bootc-build/Containerfile);
  the VM itself is defined in [`edge-ipc.yaml`](https://github.com/amosproj/amos2026ss01-zero-downtime-linux-updates/blob/main/dev-env/lima/edge-ipc.yaml).

## Prerequisites

- `limactl`
- `oras`
- `jq`
- `swtpm`
- `podman`

## Run it

0. *cd* into the project root
1. Get a disk image into ./dist - pick ONE:
   ```bash
   make pull-image PULL_REF=main      # download the prebuilt image from CI (fast)
   make image                         # OR build it locally from source
   ```
   Notice: When using *PULL_REF* to target a certain branch, replace `/` with `-`
2. Create the VM (boots the image from ./dist).
   You may need to delete a previous vm first: `limactl rm -f edge-ipc`.
   ```bash
   limactl create --name edge-ipc dev-env/lima/edge-ipc.yaml --vm-type qemu --arch x86_64 
   ```
   For the TPM to work, QEMU *must* be used!
3. Start the software TPM
   ```bash
   scripts/create_tpm.sh
   swtpm socket --tpm2 --tpmstate dir=/tmp/emulated_tpm --ctrl type=unixio,path=/tmp/emulated_tpm/swtpm-sock --log level=20 -d
   ```
   (The swtpm is forked to the background and terminates, as soon the VM is shut down once it has attached to the socket)

   Using the command above, the TPM state is saved under */tmp/emulated_tpm*. Could be useful for testing, at least good to know.
4. Source the device parameter variables and start the VM (with the vTPM attached)
   ```bash
   . scripts/tests/common_env.sh
   QEMU_SYSTEM_X86_64="qemu-system-x86_64 \
       -chardev socket,id=chrtpm,path=/tmp/emulated_tpm/swtpm-sock \
       -tpmdev emulator,id=tpm0,chardev=chrtpm \
       -device tpm-tis,tpmdev=tpm0 \
       -smbios type=1,uuid=${DEVICE_UUID},serial=${DEVICE_SERIAL} \
       -smbios type=2,serial=${DEVICE_SERIAL}" \
       limactl start edge-ipc
   ./scripts/tests/e2e_register_pending_device.sh
   ```
   (append `--log-level debug` for more verbose output from limactl)

   The script for "registering" the device simulates the TPM endorsement key uploading, so after a short delay, the device can register itself.

## Accessing the VM

Get a (implicit ssh) shell into the VM
```bash
limactl shell edge-ipc
```

Copy files to/from VM with `limactl copy` https://lima-vm.io/docs/reference/limactl_copy/
```bash
limactl copy target/debug/amos-orchestrator edge-ipc:/tmp/
```

Alternatively:
Get the ssh config for the VM and use that for explicit ssh access or scp'ing sth. to the VM:
```bash
edge_ssh_config=`limactl ls --format='{{.SSHConfigFile}}' edge-ipc`

ssh -F "$edge_ssh_config" edge-ipc 

scp -F "$edge_ssh_config" target/debug/amos-orchestrator lima-edge-ipc:/tmp/
```

Inside the VM, watch the orchestrator systemd service: 
```bash
journalctl -fu orchestrator.service
```

The host is rechable over the network via `host.lima.internal`. (When running the api server on the host its url would be `http://host.lima.internal:8080/` then)

## Stopping/deleting

Stop the VM (state preserved, can be started again): `limactl stop edge-ipc`

Delete the VM (state lost): `limactl delete edge-ipc`

## Observing logs and status

#### vm: qemu booting
follow logs of VM booting:
```
less -N +F ~/.lima/edge-ipc/serial.log
# or in color:
tail -f ~/.lima/edge-ipc/serial.log | bat --paging=never -l log
```
or just read them after the fact:
```
~/.lima/edge-ipc/serial.log
```

> Tip: for syntax highlighting of .log files use e.g. `bat` https://github.com/sharkdp/bat or a nvim plugin https://github.com/fei6409/log-highlight.nvim

#### orchestrator.service
use `journalctl`:

```bash
limactl shell edge-ipc -- journalctl -u orchestrator.service -f   # follow
limactl shell edge-ipc -- journalctl -u orchestrator.service -n200  # last 200 lines
```

## Key paths inside the VM

This is a bootc/ostree system. See
[`architecture.md`](../architecture.md)
for the full explanation of `/usr` (read-only, image), `/etc` (writable,
merged) and `/var` (writable, first-boot-populated only). In the dev VM
specifically:

| Path                                                     | What                                                                      | Notes                                                                                       |
| -------------------------------------------------------- | ------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------- |
| `/usr/libexec/amos-orchestrator`                         | The orchestrator binary shipped in the image                              | Read-only; updated atomically with the OS                                                   |
| `/var/usrlocal/bin/amos-orchestrator`                    | The binary the dev VM's service actually runs                             | Writable; populated by `make dev-deploy`. Falls back to a symlink to `/usr/libexec` on boot |
| `/etc/systemd/system/orchestrator.service.d/10-dev.conf` | Drop-in that redirects `ExecStart` at the writable path above             | Written by [`edge-ipc.yaml`](https://github.com/amosproj/amos2026ss01-zero-downtime-linux-updates/blob/main/dev-env/lima/edge-ipc.yaml); dev-only — not present in prod images        |
| `/etc/amos/config.toml`                                  | Orchestrator config (cloud URL, poll interval, inventory path, device ID) | Written when the VM is created, from [`edge-ipc.yaml`](https://github.com/amosproj/amos2026ss01-zero-downtime-linux-updates/blob/main/dev-env/lima/edge-ipc.yaml)                     |
| `/var/lib/amos/inventory.json`                           | Device inventory the agent writes on startup                              | Standard place for app state; created by the service (runs as root)                         |
| `/etc/systemd/system/orchestrator.service`               | The systemd service that runs the agent                                   | Enabled at image-build time; starts on boot                                                 |

> **Heads-up:** run the agent via systemd (it runs as root). Running
> `amos-orchestrator` by hand as your normal user fails to create `/var/lib/amos`
> because only root may write under `/var/lib`. Use
> `sudo systemctl start orchestrator.service` instead.

## Environment & Configuration Variables Reference

To successfully provision and run the local edge environment, configurations are split across host environment scripts, the Lima VM template, and the orchestrator configuration file.

### 1. Host Identity Variables (`scripts/tests/common_env.sh`)
Before launching the VM, sourcing `common_env.sh` initializes hardware parameters injected into the virtualized system management BIOS (SMBIOS):
* **`DEVICE_UUID`**: A mock unique identifier assigned to the edge node.
* **`DEVICE_SERIAL`**: A mock hardware serial string.

These variables are leveraged by the QEMU boot parameters (`-smbios type=1...`) to assign deterministic identities to the VM. This identity allows the orchestrator to register its virtual TPM (vTPM) and authenticate successfully against the simulated cloud backend.

### 2. Lima Core Variables (`edge-ipc.yaml`)
These values govern the guest environment parameters set inside the VM container context:
* **`cpus` / `memory` / `disk`**: Resource baselines (Default: 2 Cores, 4GiB RAM, 20GiB Storage) required to cleanly run the underlying bootc/Fedora IoT system layer.
* **`LIMA_CIDATA_GUEST_INSTALL_PREFIX` (Default: `/var/usrlocal`)**: Directs guest components to look under writable disk segments for hot-swapping development binaries, mitigating the constraints of the immutable read-only `/usr` file system structure.
* **`APP_CONFIG_FILE` (Default: `/etc/amos/config.toml`)**: Specifies the direct execution path used by the orchestrator systemd service unit.

### 3. Orchestrator Application Config (`orchestrator-config.toml`)
This configuration file maps the specific runtime logic of the `amos-orchestrator` binary and is automatically provisioned inside the guest space at `/etc/amos/config.toml`:

| Key | Default Value | Purpose |
| :--- | :--- | :--- |
| `cloud_url` | `"http://host.lima.internal:8080/v1"` | The absolute API endpoint route. `host.lima.internal` resolves directly back to the host machine loopback loop. |
| `poll_interval_secs` | `5` | Pacing threshold (in seconds) tracking how aggressively the device checks the cloud for new OS container or application manifest updates. |

## Iterating on the orchestrator (hot-swap)

You don't need to rebuild the OS image to test orchestrator changes. The dev
VM's systemd drop-in points the unit's `ExecStart` at
`/var/usrlocal/bin/amos-orchestrator` (writable), and `make dev-deploy`
cross-builds for the VM's arch, drops the binary, and restarts the service:

```bash
# from the project root, with the VM already running
make dev-deploy            # native build (host cargo), VM name 'edge-ipc'
make dev-deploy DEV_VM=my-vm
make dev-deploy-container   # build in a container instead (macOS / cross-arch)
```

What `dev-deploy` does:

1. Asks the running VM for its arch (`uname -m`) — this matters because your
   **host may be macOS arm64 or amd64**, and the VM may have been started with
   a different `--arch` than the host. The orchestrator must be built for the
   **VM's** arch, not the host's.
2. Builds the orchestrator. By default (`make dev-deploy`) it builds with your
   **host's own cargo** — fast and no podman, but it only produces a binary for
   the host's arch, so run it on a Linux host matching the VM (e.g. inside the
   devcontainer). On macOS or to cross-build for a different arch, use
   `make dev-deploy-container`, which builds inside a Linux `rust:1.95-slim`
   container at the right `--platform`. Either way output goes to
   `target/dev-vm-<arch>/release/`, separate from your host-native
   `target/release/`.
3. Uploads the binary into the VM with `limactl copy` (scp/sftp over the VM's
   SSH connection), then `sudo install`s it into
   `/var/usrlocal/bin/amos-orchestrator` and runs `systemctl restart`.

If the VM isn't running yet, start it as usual — on first boot, the provision
script symlinks `/var/usrlocal/bin/amos-orchestrator` to `/usr/libexec/...` so
the service starts cleanly even before you've deployed a dev binary.

## Troubleshooting

- **Connection errors in the log:** the agent polls the cloud API at the
  `cloud_url` in `config.toml` (by default the server on the host's port
  8080). Start the server if you want successful polls.
