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
- The VM image is defined in [`../../bootc/Containerfile`](../../bootc/Containerfile);
  the VM itself is defined in [`edge-ipc.yaml`](./edge-ipc.yaml).

## Prerequisites

- `limactl`
- `oras`
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
2. Create the VM (boots the image from ./dist)
   ```bash
   limactl create --name edge-ipc dev-env/lima/edge-ipc.yaml
   ```
3. Start the software TPM
   ```bash
   mkdir -p /tmp/emulated_tpm
   swtpm socket --tpm2 -d --tpmstate dir=/tmp/emulated_tpm --ctrl type=unixio,path=/tmp/emulated_tpm/swtpm-sock --log level=20
   ```
   (The swtpm is forked to the background and terminates, as soon the VM is shut down once it has attached to the socket)

   Using the command above, the TPM state is saved under */tmp/emulated_tpm*. Could be useful for testing, at important good to know.
3. Start the VM (with the vTPM attached)
   ```bash
   QEMU_SYSTEM_X86_64="qemu-system-x86_64 \
       -chardev socket,id=chrtpm,path=/tmp/emulated_tpm/swtpm-sock \
       -tpmdev emulator,id=tpm0,chardev=chrtpm \
       -device tpm-tis,tpmdev=tpm0" \
       limactl start edge-ipc
   ```
   For the TPM to work, QEMU *must* be used

## Accessing the VM

Get a (implicit ssh) shell into the VM: `limactl shell edge-ipc`

Get the ssh config for the VM and use that for explicit ssh access or scp'ing sth. to the VM:

```bash
edge_ssh_config=`limactl ls --format='{{.SSHConfigFile}}' edge-ipc`

ssh -F "$edge_ssh_config" lima-edge-ipc

scp -F "$edge_ssh_config" target/debug/amos-orchestrator lima-edge-ipc:/tmp/
```

Inside the VM, watch the orchestrator systemd service: `journalctl -fu orchestrator.service`

The host is rechable over the network via `host.lima.internal`. (When running the api server on the host its url would be `http://host.lima.internal:8080/` then)

## Stopping/deleting

Stop the VM (state preserved, can be started again): `limactl stop edge-ipc`

Delete the VM (state lost): `limactl delete edge-ipc`

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
