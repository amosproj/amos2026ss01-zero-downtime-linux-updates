# AMOS — Zero-Downtime Linux Updates

This is the project documentation for **AMOS SS 2026 — Zero-Downtime Linux
Updates**: an edge-device update system where the entire operating system is
shipped as a bootable container image and updated atomically, with automatic
rollback if an update fails to boot or fails its health checks.

At the center is the `amos-orchestrator` agent, which runs on each edge device,
polls the cloud for the OS and application versions it should be running, and
drives `bootc` and `podman` to converge the device onto that state.

## Where to start

If you are new to the project, read these in order:

1. **[Fundamentals: OS Images & bootc](./image-based-os.md)** — the core idea
   the whole project rests on: what a bootable container image is, and how a
   device is installed once and then updated forever from a single image in a
   registry. Start here.
2. **[Architecture and Design](./architecture_and_design_documentation.md)** —
   the components, the orchestrator's OS/app/ping loops, key data structures,
   and the design decisions behind them.
3. **[User documentation](./user_documentation.md)** — how to install,
   configure, and operate the orchestrator on an edge device.

## Reference by task

| I want to… | See |
| --- | --- |
| Understand the image-based OS model | [Fundamentals: OS Images & bootc](./image-based-os.md) |
| Understand how the system fits together | [Architecture and Design](./architecture_and_design_documentation.md) |
| Build the workspace, images, and containers | [Build documentation](./build_documentation.md) |
| Provision bare-metal hardware from an ISO | [ISO build & bare-metal provisioning](./bootc-build/iso.md) |
| Run and operate the orchestrator | [User documentation](./user_documentation.md) |
| Query or ship device logs | [Log API (TimescaleDB)](./log_api.md) |
| Understand the CI/CD pipeline | [CI pipeline](./ci.md) |
| Set up a local dev environment | [Development environment](./Dev_Environment.md) · [Local edge VM (Lima)](./dev-env/lima.md) |
| Read the API specs | Device / User OpenAPI specs (Swagger UI, from the docs landing page) |
| Read the Rust API reference | rustdoc crate pages (from the docs landing page) |

> The complete chapter list is always in the sidebar on the left. This page is
> just a guided entry point.

## Building these docs locally

`scripts/build-docs.sh` (run via `make`) is the single source of truth for the
docs build — the same script CI uses to publish the site, so a local build
matches what gets deployed. From the repo root:

```sh
make docs         # build the site into ./target/doc
make docs-serve   # build, then serve at http://localhost:8000 (override with DOCS_PORT)
```

Requires `cargo` and `mdbook` (`cargo install mdbook`) on your PATH. On a
machine where the workspace's Linux-only TPM crate can't compile (e.g. macOS),
set `SKIP_RUSTDOC=1` to build only the mdBook prose.

## How the published site is assembled

The `docs.yml` workflow builds and publishes on every push to `main` (PRs build
as a check only, no deploy). The site under `target/doc/` has three parts:

- **mdBook** — these Markdown files, compiled to a static site at `docs/`.
- **rustdoc** — `cargo doc` generates the Rust API reference alongside it.
- **Landing page** — `scripts/docs-landing.html` becomes `index.html` and links
  to the mdBook, the Swagger API specs, and the rustdoc crate pages.
