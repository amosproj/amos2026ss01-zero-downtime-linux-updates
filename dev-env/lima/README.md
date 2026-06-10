# Local test setup — edge device in a Lima VM

This boots a realistic **edge device** on your machinge so you can see the update
agent run end to end, without any physical hardware.

## What it is

- A [Lima](https://lima-vm.io) VM runs a **bootc / Fedora IoT** image — an
  immutable, image-based Linux that updates atomically and can roll back.
- Inside it runs our **orchestrator** (`amos-orchestrator`), the agent that
  checks the cloud for OS/app updates, applies them, and reports a device
  **inventory** back. On boot it writes the inventory to a JSON file and then
  polls the cloud API on an interval.
- The VM image is defined in [`../../rootc-build/Containerfile`](../../rootc-build/Containerfile);
  the VM itself is defined in [`edge-ipc.yaml`](./edge-ipc.yaml).

## Run it

Prerequisites (macOS): `limactl`, `oras`, and `podman` (`brew install lima oras podman`).

```bash
# From the project root.

# 1. Get a disk image into ./dist — pick ONE:
make pull-image PULL_REF=main      # download the prebuilt image from CI (fast)
make image                         # OR build it locally from source

# 2. Start the VM (boots the image from ./dist).
limactl start --name edge-ipc dev-env/lima/edge-ipc.yaml
# optionally: --arch x86_64 --vm-type qemu
# vm-type: https://lima-vm.io/docs/config/vmtype/

# 3. Watch the orchestrator do its thing.
limactl shell edge-ipc -- journalctl -u orchestrator.service -f

# 4. start the api-server and db on your host-machine (outside the vm)
podman compose -f .devcontainer/docker-compose.yml up -d mock-api-container postgres-container
# or 
limactl shell edge-ipc
```

The orchestrator starts automatically (it's a systemd service enabled in the
image). To inspect the inventory it produced:

```bash
limactl shell edge-ipc -- sudo cat /var/lib/amos/inventory.json
```

Tear down with `limactl stop edge-ipc && limactl delete edge-ipc`.

## Key paths inside the VM

This is a bootc/ostree system, so the filesystem is split: `/usr` is a
**read-only** part of the OS image, while `/etc` and `/var` are **writable** and
**persist** across OS updates.

| Path | What | Notes |
|------|------|-------|
| `/usr/local/bin/amos-orchestrator` | The orchestrator binary | `/usr/local` is a symlink to writable `/var/usrlocal`; the rest of `/usr` is read-only |
| `/etc/amos/config.toml` | Orchestrator config (cloud URL, poll interval, inventory path, device ID) | Written when the VM is created, from [`edge-ipc.yaml`](./edge-ipc.yaml) |
| `/var/lib/amos/inventory.json` | Device inventory the agent writes on startup | Standard place for app state; created by the service (runs as root) |
| `/etc/systemd/system/orchestrator.service` | The systemd service that runs the agent | Enabled at image-build time; starts on boot |

> **Heads-up:** run the agent via systemd (it runs as root). Running
> `amos-orchestrator` by hand as your normal user fails to create `/var/lib/amos`
> because only root may write under `/var/lib`. Use
> `sudo systemctl start orchestrator.service` instead.

## Troubleshooting

- **Connection errors in the log:** the agent polls the cloud API at the
  `cloud_url` in `config.toml` (by default the mock server on the host's port
  8080). Start the mock server if you want successful polls.


### TPM

Lima/Virtualization.framework does **not** expose TPM. When the TPM-backed
device identity work needs a target, a libvirt + swtpm profile could be added.
