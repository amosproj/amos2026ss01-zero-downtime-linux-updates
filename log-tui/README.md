# amos-log-tui

A small [ratatui](https://ratatui.rs) TUI that shows the AMOS device-management logs live
during a demo: a device sidebar, a colour-coded live log tail, and on-the-fly filters.

It is a **user-API client** — it only talks to `GET /v1/devices` and the SSE stream
`GET /v1/logs/stream`, using the same long-lived dev JWT the Bruno collection and the demo
notebooks use. Wire types (`LogEvent`, `LogLevel`, `Device`, …) are reused directly from
`amos-common`, so the client always matches the server.

## Run

```sh
# Local api-server on :8080
AMOS_JWT="<dev jwt>" cargo run -p amos-log-tui -- --base-url http://localhost:8080/v1

# A demo run on the OpenStack host
AMOS_JWT="<dev jwt>" cargo run -p amos-log-tui -- \
  --base-url http://float-172-017-069-035.cc.rrze.net/run1/v1 --device 3
```

The JWT can also be passed with `--jwt`. Grab the token from
`demo/bruno/environments/<env>.bru` (`jwt:` var).

### Flags

| Flag | Env | Default | Meaning |
|---|---|---|---|
| `--base-url` | `AMOS_BASE_URL` | `http://localhost:8080/v1` | User API base, incl. `/v1` |
| `--jwt` | `AMOS_JWT` | *(empty)* | Bearer token for the user API |
| `--device <id>` | | | Pre-select a device on startup |
| `--level <all\|trace\|…\|fatal>` | | `info` | Initial minimum severity |
| `--max-logs <n>` | | `2000` | Ring-buffer size |

## Keys

| Key | Action |
|---|---|
| `0` / `1` / `2` / `3` | Min level: all / info / warn / error (reconnects the stream) |
| `j` / `k` (or ↓ / ↑) | Select next / previous device (reconnects, filtered by `device_id`) |
| `a` (or ←) | All devices (clear the device filter) |
| `c` | Clear the on-screen log buffer |
| `q` / `Esc` / `Ctrl-C` | Quit |

Changing the level or the selected device tears down the SSE connection and reopens it with new
query params — the brief `● reconnecting` in the footer is expected.

## How it works

- `client.rs` — `fetch_devices` + `spawn_stream`. The stream task reads
  `reqwest::Response::bytes_stream()` through `eventsource-stream`, deserializes each `data:`
  payload into an `amos_common::entities::LogEvent`, and forwards it over an mpsc channel.
  Each subscription carries an `epoch`; the app ignores messages from a torn-down stream.
- `app.rs` — state + keybindings; every filter change funnels through `reconnect()`.
- `ui.rs` — sidebar + colour-coded log tail + footer/status.
- `main.rs` — terminal setup via `ratatui::init()` / `ratatui::restore()` and the
  `tokio::select!` loop between key events and the log channel.
