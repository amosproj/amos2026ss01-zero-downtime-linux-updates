## ISO Building & Bare-Metal Provisioning

To deploy the system onto bare-metal Edge IPC hardware for the first time, you must generate an installation ISO. This installer packages our bootable container image (`-system`) into an unattended Anaconda installation media.

### 1. How it Works

* **Blueprint Configuration:** The disk layout, user profiles, and post-installation hooks are controlled via the `bootc-image-builder` configurations and automated Kickstart scripts.
* **One-Time Provisioning:** The ISO is used exactly once per hardware unit to provision the disk partitions and pull down the initial deployment. From that point forward, the OS updates itself atomically over the air (OTA) using `bootc` and the Orchestrator loop.

## 2. On-disk layout (bootc / Fedora / ostree)

The edge OS is a **bootc** image (Fedora bootc). The filesystem is split into
three regions with very different update semantics:

| Region | Mutability | Update behaviour |
| --- | --- | --- |
| `/usr` | read-only | shipped in the image; replaced atomically on every OS update; rolled back as a unit |
| `/etc` | writable | persists across updates; 3-way merged against the image's `/etc` so admin overrides survive |
| `/var` | writable | persists across updates; **populated from the image only on first boot** — later image changes to `/var` are *not* propagated |

This `/var` behaviour is the main footgun: paths like `/usr/local` and `/opt`
are symlinks to `/var/usrlocal` and `/var/opt` on Fedora bootc, so anything
installed there at image-build time effectively becomes a one-shot copy. The
orchestrator therefore lives in a true read-only path so OS updates actually
replace it.

| Path | Contents | Why this path |
| --- | --- | --- |
| `/usr/libexec/amos-orchestrator` | the orchestrator binary | `/usr` is read-only and atomic with OS updates; `/usr/libexec` is the [FHS](https://refspecs.linuxfoundation.org/FHS_3.0/fhs/ch04s07.html) location for system-service binaries not meant to be invoked from `$PATH` |
| `/etc/amos/config.toml` | orchestrator config (cloud URL, poll interval, device ID, inventory path) | `/etc` is writable and merged across updates, so admin overrides persist |

The systemd unit (`/etc/systemd/system/orchestrator.service`) ships in the
image and points `ExecStart` at `/usr/libexec/amos-orchestrator`. In the dev
VM, a systemd drop-in overrides this to a writable path — see
[`dev-env/lima/README.md`](../dev-env/lima/README.md).

### 3. Building the ISO Locally

You can invoke the local build environment shortcuts configured via the project's build tooling:

```bash
make iso
```

> We use `bootc-image-builder` <https://osbuild.org/docs/bootc/>
> we do not use the newer osbuild.org `image-builder`: - "container-native ISO contract", bootloader/kernel/GRUB defined *inside* container instead of letting anaconda installer do that during install - <https://github.com/ondrejbudai/bootc-isos> - seems to not be mature/stable, only noted here for future evaluation.

### 4. Automated CI/CD Builds

The ISO is automatically compiled via the GitHub Actions pipeline (iso-image.yml) on specific event branches or releases.

Automatic Triggers: Fires on any tag push, main branch push, or branches matching whole-segment keywords like iso, ci, or release.

Outputs: The resulting AMD64 .iso is exported directly as a GitHub Actions workflow artifact.

### 5. Triggering via GitHub CLI (gh)

To manually kick off an ISO compilation targeting a specific container image revision without relying on push events, execute:

```bash
# Build the installer ISO from the latest container image
gh workflow run iso-image.yml -f image_ref=latest

# Download the compiled .iso file from the subsequent run
gh run download <run-id>
```
