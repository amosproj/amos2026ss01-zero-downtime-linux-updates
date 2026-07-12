# Bruno collection — AMOS Device Management API

Runnable API docs + the scripted live demo.

1. Install [Bruno](https://www.usebruno.com/) (v2.x or newer — the log stream
   request needs SSE support).
2. *Open Collection* → select this folder (`api/bruno`).
3. Pick the environment for your demo run (`demo-run1` / `demo-run2` /
   `demo-run3`, or `local` against a locally running `api-server`).
4. Paste the TPM endorsement key handed over by the demo operator into the
   `edge3EndorsementKey` environment variable — it is regenerated every time a
   run is prepared, so it is the one value that is never pre-filled (format:
   see *Demo Flow → 04 Register Edge 3*). `edge3Serial` is already set to the
   scanner IPC's fixed serial.
5. For the live demo, work through the **Demo Flow** folder top to bottom —
   each request documents what to show and what to expect.

The **User API** and **Device API** folders mirror the router split in
`api-server` (`/v1/*` with a user JWT vs `/v1/device/*` with a device
JWT) and cover every endpoint, so the collection also serves as an API
reference and playground.

Each demo run has its own API server (same host, different path segment) and
its own environment file; for an additional run, duplicate one of the
`environments/demo-run*.bru` files and change the run segment in `baseUrl`.

Setting up the runs, edge VMs and the `edge3EndorsementKey` hand-off is the
**demo operator's** job — see [`demo/edge/README.md`](../../demo/edge/README.md).
