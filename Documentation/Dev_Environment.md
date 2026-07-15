# Dev Environment

This repository provides a DevContainer definition for a standardized environment.

The recommended setup for development on this project therefore is `Microsoft Visual Studio Code` with the `Dev Containers` extension.

Inside the dev container, an instance of the api server can be reached under [http://api-container/](). Try it out with `curl http://api-container/v1/devices`!

## Setup

To get started, follow the [Microsoft guide](https://code.visualstudio.com/docs/devcontainers/containers#_installation) on setting up Dev Containers.
Also setup [Git credential sharing](https://code.visualstudio.com/remote/advancedcontainers/sharing-git-credentials) into the container.

Now clone the repository and open it in VS Code. You should be automatically prompted by a notification in the bottom right to `Reopen in Container`.
After doing so and waiting for it to finish (building the container the first time can take a few minutes), the whole environment should be set up and ready to go.

## Caveats

Dev Containers should be viewed as transient and read-only. Do not make changes to your running container, always change or extend the definition and rebuild to ensure replicability. When changing any file in the `.devcontainer/` folder also change the `devcontainer.json` file, so people get prompted to rebuild their containers when pulling.

## Edge VM (Lima)

see [dev-env/lima.md](dev-env/lima.md)

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

### Cargo tooling

The SBOM and license deliverables (in `Deliverables/sprint-*/`) are generated with the following cargo tools:

| Tool | Purpose | Output |
| --- | --- | --- |
| [`cargo-sbom`](https://github.com/psastras/sbom-rs) | Generate the SPDX SBOM | `sbom.spdx.json` |
| [`cargo-cyclonedx`](https://github.com/cyclonedx/cyclonedx-rust-cargo) | Generate the CycloneDX SBOM (per crate) | `SBOM-cyclonedx/*.cdx.xml` |
| [`cargo-about`](https://github.com/EmbarkStudios/cargo-about) | Collect dependency licenses (config: `about.toml`, template: `about.hbs`) | `license.html` |
| [`cargo-deny`](https://github.com/EmbarkStudios/cargo-deny) | Lint dependencies for advisories, license and source policy (config: `deny.toml`) | — |

Install and run them with:

```bash
cargo install cargo-sbom cargo-cyclonedx cargo-about cargo-deny

# SPDX SBOM
cargo sbom > sbom.spdx.json

# CycloneDX SBOM (one file per crate)
cargo cyclonedx

# License report
cargo about generate about.hbs > license.html

# Dependency / license / advisory checks
cargo deny check
```
