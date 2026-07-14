# Zero-Downtime Linux Updates

> AMOS SS 2026 · Project 01

An update system for fleets of Linux edge devices (IPCs) where the **entire
operating system ships as a bootable container image**. Devices are provisioned
once, then updated forever by pulling a new image — each OS update is atomic and
staged, and the **OS rolls back automatically** if a new version fails to boot or
fails its health checks. That is the zero-downtime guarantee.

## How it works

- **The OS is a container image.** We build a bootable OS image with
  [`bootc`](https://bootc.dev/) and publish it to GHCR, the same way you'd build
  and push an application container.
- **An on-device agent drives updates.** `amos-orchestrator` runs on each device,
  polls the cloud API for the OS and application versions it should be running,
  and invokes `bootc` and `podman` to converge onto that state.
- **The cloud holds the desired state.** A user manages the fleet through the
  cloud API; the cloud persists per-device state in PostgreSQL and hands each
  device its target state on request.

```
   User ──▶ Cloud API ──▶ PostgreSQL
                 ▲
                 │ poll desired state / report status
                 ▼
   Edge device: amos-orchestrator ──▶ bootc   (OS updates)
                                  └─▶ podman  (app containers)
                 ▲
                 │ pull OS & app images
              GHCR (product source)
```

## Documentation

Full documentation — fundamentals, architecture, build, and user guides — is
published at:

**https://amosproj.github.io/amos2026ss01-zero-downtime-linux-updates/**

New to the project? Start with **Fundamentals: OS Images & bootc**, then
**Architecture and Design**.

## Repository layout

| Path | What it is |
| --- | --- |
| `orchestrator/` | The on-device agent binary (`amos-orchestrator`) |
| `api-server/` | Cloud API server binary (`amos-api-server`) |
| `common/` | Shared library crate (API types, DTOs, download manager) |
| `bootc-build/` | Containerfile, systemd unit, and greenboot checks for the OS image |
| `Documentation/` | Project documentation (mdBook source) |
| `scripts/` | Build, test, and helper scripts |
| `dev-env/` | Local edge-device VM (Lima) and dev tooling |

## Try it out

To boot a test edge device in a local VM and watch the update agent run, see
[`dev-env/lima/README.md`](./dev-env/lima/README.md). It walks through what the
setup does, how to start it, and the key paths inside the VM.

## Building

```sh
git clone https://github.com/amosproj/amos2026ss01-zero-downtime-linux-updates.git
cd amos2026ss01-zero-downtime-linux-updates
make setup          # one-time: commit template + DCO sign-off hook
cargo build         # build all workspace crates
```

See the [Build documentation](https://amosproj.github.io/amos2026ss01-zero-downtime-linux-updates/docs/build_documentation.html)
for images, containers, and deployment.

## Project links

- [Feature board](https://github.com/orgs/amosproj/projects/97)
- [Imp-squared backlog](https://github.com/orgs/amosproj/projects/101/views/1)
- [Documentation site](https://amosproj.github.io/amos2026ss01-zero-downtime-linux-updates/)

## Contributing

Please read [CONTRIBUTING.md](./CONTRIBUTING.md) for our development workflow,
commit conventions, and how to submit pull requests. This project uses the
[Developer Certificate of Origin](./DCO) — all commits must be signed off
(`git commit -s`). Run `make setup` after cloning to configure your local
environment.

## License

See [LICENSE](./LICENSE).
