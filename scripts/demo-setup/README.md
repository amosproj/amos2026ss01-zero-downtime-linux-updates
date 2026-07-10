# Demo operator runbook

Everything the **demo operator** does to bring a demo run into its start
state and hand it off to the presenter (PO). The PO drives the actual demo
from the Bruno collection in [`demo/bruno`](../../demo/bruno) and never needs
any of the tooling here — they only need three things from you:

1. **which run** to use (`run1` / `run2` / `run3` → Bruno env `demo-run1` …),
2. the **serial number** of the edge left unregistered for the live demo,
3. that edge's **TPM endorsement key**, as a ready-to-paste JSON string.

See [Hand-off to the PO](#4-hand-off-to-the-po) for how to produce 2 and 3.

## Layout of a demo

- **One OpenStack host VM** (`amos-edge-host`) runs the nested Lima edge VMs
  of **all** runs side by side: 3 runs × 3 edges = 9 Lima VMs. An optional
  identical `amos-edge-backup` host can be prepared to fail over to.
- **Each run has its own API server**, reachable at
  `<API_BASE>/<run>/v1`. Because each run is isolated, the same edge
  UUIDs/serial are reused across runs — the server disambiguates devices by
  UUID + TPM endorsement key.
- Per run, **edges 1 and 2 are pre-registered**; **edge 3 is left
  unregistered on purpose** — registering it live *is* the demo.

### Fixed identities

| | value |
|---|---|
| edge 1 UUID | `019f4785-419a-7060-bc3c-d71c75099ac2` |
| edge 2 UUID | `019f4785-419a-777a-9dba-0a79ba5809ef` |
| edge 3 UUID | `019f4785-419a-7aae-b79d-f0ad81510156` |
| serial (all edges, all runs) | `BIOS-SERIAL-1337-AMOS-TEST-VM` |

The serial is the SMBIOS system/board serial injected via QEMU
(`scripts/dev_vm_run.sh`); it is **not** the device UUID (that is a separate
SMBIOS field). A shared serial is fine because pending registrations match on
serial **+** endorsement key, and each edge has a distinct TPM key.

## Prerequisites (on your machine)

- `openstack` CLI, authenticated (source your `openrc` / `clouds.yaml`)
- `jq`, `ssh`, `curl`, `openssl`
- Reachability of the **FAU-Intern** network (VPN)
- An ssh key matching the `amos-developer-keys` OpenStack keypair loaded
  (ssh-agent or default key)

> **`API_BASE`.** The scripts default to
> `http://float-172-017-069-035.cc.rrze.net`. If your API servers are reachable under
> a different host export `API_BASE` to that host — **without** the `/<run>/v1`
> suffix, which the scripts append. It must match the `baseUrl` the PO uses in Bruno.

## 1. One-time: prepare the host VM(s)

Clones `amos-edge-host` from the `amos-edge-base` snapshot (skipped if it
already exists), updates the repo checkout baked into the image, and pulls
the requested disk image so the nested edge VMs boot the right OS version.

```bash
./prepare_edge_vms.sh <image-tag> [host-vm-name ...]

./prepare_edge_vms.sh main                                   # primary host
./prepare_edge_vms.sh main amos-edge-host amos-edge-backup   # + backup host
```

`<image-tag>` is the GHCR disk-image tag passed to `make pull-image` as
`PULL_REF` (branch/release tag without the arch suffix, e.g. `main`).

Overridable via env: `HOST_VM`, `OS_IMAGE`, `OS_FLAVOR`, `OS_KEY_NAME`,
`OS_NETWORK`, `OS_SECURITY_GROUP`, `SSH_USER`, `REPO_DIR`, `GIT_REF`
(see the script header for details).

## 2. Per demo: boot the edge VMs and pre-register edges 1 & 2

Boots the nested Lima edge VMs (via `make demo-edge` over ssh, each with a
fresh emulated TPM) and registers edges 1 & 2 with each run's API server.
Edge 3 is booted but left unregistered.

```bash
./prepare_edge_demo.sh all           # all three runs (run1 run2 run3)
./prepare_edge_demo.sh run1          # just one run
```

Re-running **recreates** the Lima VMs with fresh TPMs. Re-registering a
device the server already knows creates a **duplicate** device entry, so if
you re-prepare a run, **reset that run's API server first**.

## 3. Smoke-test before handing off

Confirm each run's API server shows its two pre-registered devices:

```bash
source ../tests/common_env.sh   # exports $JWT (the shared dev token)
curl -s <API_BASE>/run1/v1/devices \
  -H "Authorization: Bearer $JWT" | jq '.data | length'   # expect 2
```

`$JWT` is the same token baked into the Bruno environments.

## 4. Hand-off to the PO

Give the PO the **run name** (→ they pick the matching Bruno environment) and
edge 3's **serial** + **endorsement key**. Produce the endorsement key as a
JSON string (quotes included, trailing newline preserved — the server matches
it byte-for-byte):

```bash
HOST_IP=$(openstack server show amos-edge-host -f json -c addresses \
  | jq -r '.addresses["FAU-Intern"][0]')

# For run1's edge 3 (Lima VM name: run1-edge-3):
ssh debian@"$HOST_IP" \
  "limactl shell run1-edge-3 -- \
     sudo tpm2_readpublic -c 0x81010001 -f pem -o /dev/stdout \
   | openssl rsa -pubin 2>/dev/null" | jq -Rs .
```

The PO pastes that output verbatim into the Bruno `edge3EndorsementKey`
environment variable and the serial into `edge3Serial`, then runs
*Demo Flow → 04 Register Edge 3*.

### Alternative: register edge 3 yourself, live

If you'd rather not hand over the key, register edge 3 during the demo with:

```bash
./prepare_edge_demo.sh run1 --register-only        # default edge: 3
```

This reads the TPM key, POSTs the pending registration, and waits (up to 90s)
for the orchestrator to self-register — the PO then just re-sends
*Demo Flow → 05 Devices After Registration* until the third device appears.

## Troubleshooting

- **Edge 3 doesn't self-register within ~a minute.** Restart its
  orchestrator:
  `ssh debian@$HOST_IP "limactl shell run1-edge-3 -- sudo systemctl restart orchestrator.service"`.
- **Duplicate devices after re-preparing a run.** Reset that run's API
  server, then re-run `prepare_edge_demo.sh <run>`.
- **`could not read endorsement key`.** The Lima VM isn't up yet — re-run
  step 2 for that run, or check `limactl list` on the host.
- **Primary host wedged.** Prepare `amos-edge-backup` (step 1) and point
  the runs at it via `HOST_VM=amos-edge-backup`.
