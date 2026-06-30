# hello-world

Minimal AMOS application image used to exercise the orchestrator's podman log pipeline.

On startup it prints a banner with its version, hostname, and all environment variables.
It then emits a heartbeat to **stdout** every `$HEARTBEAT_INTERVAL` seconds (default 5)
and a probe to **stderr** every 6th beat — mapping to `LogLevel::Info` and
`LogLevel::Error` respectively in `orchestrator/src/podman/log_registry.rs`.

## Build & run locally

```sh
podman build -t hello-world:test applications/hello-world

# Interactive — prints banner + heartbeats
podman run --rm \
  -e GREETING="Hello" \
  -e NAME="AMOS" \
  -e HEARTBEAT_INTERVAL=3 \
  hello-world:test

# Detached — tail logs (mirrors what the orchestrator does)
podman run -d --name hw -e NAME=AMOS hello-world:test
podman logs -f hw
podman rm -f hw
```

## Wiring into the orchestrator (end-to-end test)

Point the mock server at the pushed GHCR image by adding an entry to
`api-mock-server/src/api_v1/db/application_configs.rs` with:

```rust
image: "ghcr.io/amosproj/amos2026ss01-zero-downtime-linux-updates-system-hello-world:latest".into(),
environment: Some([("NAME".into(), "AMOS".into())].into()),
```

The orchestrator's `loop_apps.rs` will pull, run, and tail the container automatically.

## Versioning

The image version is read from `VERSION` in this directory and applied as the
`APP_VERSION` build-arg and as the semver image tag pushed to GHCR.
