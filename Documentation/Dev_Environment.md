# Dev Environment

This repository provides a DevContainer definition for a standardized environment.

The recommended setup for development on this project therefore is ```Microsoft Visual Studio Code``` with the ```Dev Containers``` extension.

Inside the dev container, an instance of the mock api server can be reached under [http://mock-api-container/](). Try it out with ```curl http://mock-api-container/v1/catalog```!

## Setup

To get started, follow the [Microsoft guide](https://code.visualstudio.com/docs/devcontainers/containers#_installation) on setting up Dev Containers.
Also setup [Git credential sharing](https://code.visualstudio.com/remote/advancedcontainers/sharing-git-credentials) into the container.

Now clone the repository and open it in VS Code. You should be automatically prompted by a notification in the bottom right to ```Reopen in Container```.
After doing so and waiting for it to finish (building the container the first time can take a few minutes), the whole environment should be set up and ready to go.

## Caveats

Dev Containers should be viewed as transient and read-only. Do not make changes to your running container, always change or extend the definition and rebuild to ensure replicability. When changing any file in the ```.devcontainer/``` folder also change the ```devcontainer.json``` file, so people get prompted to rebuild their containers when pulling.

## Edge VM (Lima)

The DevContainer covers the **cloud side** (mock API + Postgres). It does *not*
boot a real Fedora bootc OS, so it cannot exercise `bootc_wrapper`,
`podman_wrapper`, greenboot, or cosign signature verification end-to-end. For
those code paths you need a real VM.

We use [Lima](https://lima-vm.io/) as the cross-platform VM runner because a
single template works on Linux (QEMU/KVM), macOS arm64 (Virtualization.framework),
and Windows via WSL2 (QEMU). The disk image itself is built once by
`bootc-image-builder` from `rootc-build/Containerfile`.

### Quickstart (no local image build)

Pull the latest released disk image and boot it. No podman / bootc tooling
required on the host beyond Lima.

1. Install Lima:
   - macOS: `brew install lima`
   - Linux: see [Lima install docs](https://lima-vm.io/docs/installation/)
   - Windows: install Lima inside a WSL2 Ubuntu 24.04 distro
2. Start the mock cloud (devcontainer compose, or directly):
   ```bash
   docker compose -f .devcontainer/docker-compose.yml up -d mock-api-container postgres-container
   ```
   The mock-api is now bound to `127.0.0.1:8080` on the host.
3. Boot the VM:
   ```bash
   limactl start --name edge-ipc dev-env/lima/edge-ipc.yaml
   limactl shell edge-ipc -- journalctl -u orchestrator.service -f
   ```

From inside the VM the host is reachable as `host.lima.internal` — the
cloud-init seed has already pointed `cloud_url` at
`http://host.lima.internal:8080/api/v1`. Verify with
`curl http://host.lima.internal:8080/v1/catalog` from inside the VM.

### Inner loop (local image build)

When iterating on `rootc-build/Containerfile`, `orchestrator.service`, or the
embedded orchestrator binary, build the disk image locally so changes show up
on the next boot instead of waiting for a release tag.

```bash
make image                                              # ~5 min
limactl start --reset --name edge-ipc dev-env/lima/edge-ipc.yaml
```

`make image` runs `bootc-image-builder` and writes `dist/qcow2/disk.qcow2` and
`dist/raw/disk.raw`. The Lima template prefers the local `dist/` paths over
the published release artifact.

Requirements for `make image`: rootful `podman` on Linux (or
`podman machine` with rootful mode on macOS), and ~10 GB free disk in `dist/`.

### Windows

Run Lima from inside WSL2 (Ubuntu 24.04 recommended). The template's
`vmType: "vz"` is silently ignored and Lima falls back to QEMU, so no config
change is needed.

### TPM

Lima/Virtualization.framework does **not** expose TPM. When the TPM-backed
device identity work needs a target, a libvirt + swtpm profile will be added
(Linux only). vTPM is out of scope for this default dev VM.

### CI artifacts

The `Edge Disk Image` workflow (`.github/workflows/disk-image.yml`) publishes
`amos-edge-<tag>-{amd64,arm64}.{qcow2,raw}.xz` as draft GitHub release
attachments on each `sprint-*-release` tag. The Lima template's release
fallback `location:` points at those URLs (replace `sprint-XX-release` with
the actual tag).

## SBOM

To keep the SBOM up-to-date, the following command can be used to list all (top level) dependencies and compare them with the SBOM sheet:

```bash
cargo tree --depth 1 --prefix none --edges normal | grep -v '^amos' | grep -v '(*)$' | sort | uniq
```
