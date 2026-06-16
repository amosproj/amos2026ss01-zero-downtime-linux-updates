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

Prerequisites (macOS): `limactl` and `podman` (`brew install lima podman`).
For `make pull-image` you also need `oras` and `jq` (`brew install oras jq`).

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
```

The orchestrator starts automatically (it's a systemd service enabled in the
image). To inspect the inventory it produced:

```bash
limactl shell edge-ipc -- sudo cat /var/lib/amos/inventory.json
# or do it manually after entering the vm with
limactl shell edge-ipc
```

Tear down with `limactl stop edge-ipc && limactl delete edge-ipc`.

## Viewing logs

The quick path is `journalctl` directly:

```bash
limactl shell edge-ipc -- journalctl -u orchestrator.service -f   # follow
limactl shell edge-ipc -- journalctl -u orchestrator.service -n200  # last 200 lines
```

Dev images also ship [`lazyjournal`](https://github.com/Lifailon/lazyjournal), a
TUI for browsing systemd journals — handy for hopping between units, scrolling
back, and searching:

```bash
limactl shell edge-ipc -- lazyjournal
```

Filter to a single unit (e.g. the orchestrator) with `/` inside the TUI, or
pick from the unit list on the left. `lazyjournal` is only baked into images
built with `DEV_MODE=true` — that's what `make image` does, but the CI image
fetched by `make pull-image` is built prod-like and won't have it.

## Key paths inside the VM

This is a bootc/ostree system. See
[`../../Documentation/architecture.md`](../../Documentation/architecture.md)
for the full explanation of `/usr` (read-only, image), `/etc` (writable,
merged) and `/var` (writable, first-boot-populated only). In the dev VM
specifically:

| Path                                                     | What                                                                      | Notes                                                                                       |
| -------------------------------------------------------- | ------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------- |
| `/usr/libexec/amos-orchestrator`                         | The orchestrator binary shipped in the image                              | Read-only; updated atomically with the OS                                                   |
| `/var/usrlocal/bin/amos-orchestrator`                    | The binary the dev VM's service actually runs                             | Writable; populated by `make dev-deploy`. Falls back to a symlink to `/usr/libexec` on boot |
| `/etc/systemd/system/orchestrator.service.d/10-dev.conf` | Drop-in that redirects `ExecStart` at the writable path above             | Written by [`edge-ipc.yaml`](./edge-ipc.yaml); dev-only — not present in prod images        |
| `/etc/amos/config.toml`                                  | Orchestrator config (cloud URL, poll interval, inventory path, device ID) | Written when the VM is created, from [`edge-ipc.yaml`](./edge-ipc.yaml)                     |
| `/var/lib/amos/inventory.json`                           | Device inventory the agent writes on startup                              | Standard place for app state; created by the service (runs as root)                         |
| `/etc/systemd/system/orchestrator.service`               | The systemd service that runs the agent                                   | Enabled at image-build time; starts on boot                                                 |

> **Heads-up:** run the agent via systemd (it runs as root). Running
> `amos-orchestrator` by hand as your normal user fails to create `/var/lib/amos`
> because only root may write under `/var/lib`. Use
> `sudo systemctl start orchestrator.service` instead.

## Iterating on the orchestrator (hot-swap)

You don't need to rebuild the OS image to test orchestrator changes. The dev
VM's systemd drop-in points the unit's `ExecStart` at
`/var/usrlocal/bin/amos-orchestrator` (writable), and `make dev-deploy`
cross-builds for the VM's arch, drops the binary, and restarts the service:

```bash
# from the project root, with the VM already running
make dev-deploy            # defaults to VM name 'edge-ipc'
make dev-deploy DEV_VM=my-vm

# then watch it run
limactl shell edge-ipc -- journalctl -u orchestrator.service -f
```

What `dev-deploy` does:

1. Asks the running VM for its arch (`uname -m`) — this matters because your
   **host may be macOS arm64 or amd64**, and the VM may have been started with
   a different `--arch` than the host. The orchestrator must be built for the
   **VM's** arch, not the host's.
2. Builds inside a Linux `rust:1.95-slim` container at the right `--platform`
   (so devs don't need a Linux cross-toolchain on macOS). Output goes to
   `target/dev-vm-<arch>/release/` to keep it separate from your host-native
   `target/release/`.
3. Copies the binary into `/tmp/lima/` on the host (lima mounts this writable
   into the VM at the same path), then `sudo install`s it into
   `/var/usrlocal/bin/amos-orchestrator` and runs `systemctl restart`.

If the VM isn't running yet, start it as usual — on first boot, the provision
script symlinks `/var/usrlocal/bin/amos-orchestrator` to `/usr/libexec/...` so
the service starts cleanly even before you've deployed a dev binary.

## Troubleshooting

- **Connection errors in the log:** the agent polls the cloud API at the
  `cloud_url` in `config.toml` (by default the mock server on the host's port
  8080). Start the mock server if you want successful polls.

### TPM

Lima/Virtualization.framework does **not** expose TPM. When the TPM-backed
device identity work needs a target, a libvirt + swtpm profile could be added.
