# Demo setup usage

## (Initial) setup

The api server binary needs to be copied to the server first (eg. from a local build, CI artifact,
whatever - we don't have useful CI for this right now):

```console
scp api-mock-server <api-server-host>:/home/almalinux
```

Then on the host, clone the repo and build the image:

```console
git clone https://github.com/amosproj/amos2026ss01-zero-downtime-linux-updates.git
cd amos2026ss01-zero-downtime-linux-updates/demo-infra
./build_server_image.sh
```

## Managing instances for the runs

Start instance 1:

```console
cd amos2026ss01-zero-downtime-linux-updates/demo-infra
podman compose -f run1/compose.yaml up -d
```

Analogously, use `-f run2/compose.yaml` for the run 2 instance and run3 for the last run.

In case we need to "reset" the data during preparation, do a compose down of the instance and then:

```console
podman volume rm runX_postgres_data runX_timescale_data
```
