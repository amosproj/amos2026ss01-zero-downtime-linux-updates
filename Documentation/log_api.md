# Log API (TimescaleDB)

The API server connects to two PostgreSQL databases:

- The **main database** (`database_url`) stores device/tenant/application
  metadata, as before.
- A second **TimescaleDB database** (`timescale_database_url`) stores
  high-volume device and application container logs in two
  [hypertables](https://docs.timescale.com/use-timescale/latest/hypertables/):
  `device_logs` and `application_logs`.

There is **no foreign-key relationship** between the two databases:
`device_id`/`application_id` columns in the log tables reference rows in the
main database by id, but this cannot be enforced by the database itself.

## Schema

### `device_logs`

| Column      | Type          | Notes                                   |
| ----------- | ------------- | --------------------------------------- |
| `time`      | `timestamptz` | Part of the composite primary key, hypertable partitioning column |
| `id`        | `uuid`        | Part of the composite primary key, generated server-side (UUIDv7) |
| `device_id` | `integer`     | References a device in the main database |
| `level`     | `text`        | One of `trace`, `debug`, `info`, `warn`, `error`, `fatal` |
| `message`   | `text`        | The log message |
| `source`    | `text`, nullable | Optional source identifier (e.g. process/unit name) |

### `application_logs`

Same as `device_logs`, plus an `application_id integer` column referencing an
application in the main database.

### `LogLevel` ordering

The six levels have a defined severity order, used for the minimum-severity
filter on the SSE stream:

```
trace < debug < info < warn < error < fatal
```

## Endpoints

All endpoints are under `/v1` and require the same JWT bearer authentication
as the rest of the API.

### `POST /v1/logs/devices?device_uuid=<uuid>`

Publish one or more log lines about a device (e.g. from the orchestrator
agent itself).

- `device_uuid` query parameter is **required** — `422` if missing.
- `entries` must be **non-empty** — `422` if empty.
- `404` if no device with the given `device_uuid` exists.

Request body:

```json
{
  "entries": [
    {
      "time": "2026-06-12T10:00:00Z",
      "level": "info",
      "message": "Orchestrator started",
      "source": "amos-orchestrator"
    },
    {
      "time": null,
      "level": "warn",
      "message": "Disk usage above 80%",
      "source": null
    }
  ]
}
```

`time` is optional; if omitted, the server uses the current time. `source` is
optional.

Response: `201 Created` with the list of stored entries:

```json
[
  {
    "id": "019621e3-...-7000-...",
    "time": "2026-06-12T10:00:00Z",
    "device_id": 1,
    "level": "info",
    "message": "Orchestrator started",
    "source": "amos-orchestrator"
  },
  {
    "id": "019621e3-...-7001-...",
    "time": "2026-06-12T10:00:01Z",
    "device_id": 1,
    "level": "warn",
    "message": "Disk usage above 80%",
    "source": null
  }
]
```

### `POST /v1/logs/applications?device_uuid=<uuid>`

Publish one or more log lines for an application container running on a
device.

Same validation rules as above (`device_uuid` required, `entries`
non-empty, `404` for unknown device), plus the request body carries an
`application_id`:

```json
{
  "application_id": 5,
  "entries": [
    {
      "time": null,
      "level": "error",
      "message": "Connection refused",
      "source": "my-app"
    }
  ]
}
```

Response: `201 Created` with the list of stored entries, each including
`device_id` and `application_id`.

### `GET /v1/logs/stream`

Server-Sent Events (SSE) stream of incoming logs (both device and
application logs), filterable via optional query parameters:

- `device_id` — only events for this device.
- `application_id` — only application-log events for this application
  (device-log events are excluded entirely if this filter is set).
- `level` — minimum severity (`level <= event.level`), e.g. `level=warn`
  returns `warn`, `error` and `fatal` events.

Each event is sent as a `message` event with a JSON payload tagged by
`kind`:

```
event: message
data: {"kind":"device","id":"...","time":"...","device_id":1,"level":"warn","message":"...","source":null}

event: message
data: {"kind":"application","id":"...","time":"...","device_id":1,"application_id":5,"level":"error","message":"...","source":"my-app"}

```

This stream is **in-memory only**: it broadcasts entries as they are
inserted by the two publish endpoints above, with no replay/history. It is
intended for "live tail" use cases; querying historical logs is not yet
supported.

Because `/v1/logs/stream` sits behind the global JWT middleware,
`curl -N -H "Authorization: Bearer <token>" .../v1/logs/stream` works, but a
browser's native `EventSource` API (which cannot set request headers) cannot
be used directly against this endpoint.

## Configuration

The TimescaleDB connection is configured separately from the main database:

```toml
# config.toml
timescale_database_url = "postgres://app:4M0S@127.0.0.1:5433/amos_timeseries"
```

or via the `APP_TIMESCALE_DATABASE_URL` environment variable. The URL must
start with `postgres://` — there is no SQLite fallback, since hypertables are
PostgreSQL/TimescaleDB-specific.
