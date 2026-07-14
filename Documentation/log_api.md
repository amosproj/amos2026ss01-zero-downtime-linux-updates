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

All device-facing endpoints are typically located under `/v1` and require a valid JWT token signed by the registered TPM as an `Authorization: Bearer <token>` header. User-facing (administrative) endpoints provide querying capabilities.  

### `POST /v1/device/logs`

Publish one or more log lines about a device (e.g. from the orchestrator agent itself). This single endpoint handles both OS-level and application-level logs.

- `application_id` (query parameter, optional): The ID of the application producing these logs. Leave this empty to signal that these logs come from the OS itself.
- The device identity is securely derived from the provided JWT bearer token.

Request body:
An array of log entry objects. `time` (ISO timestamp) and `source` are optional; `level` and `message` are required.

```json
[
  {
    "time": "2026-06-12T10:00:00Z",
    "level": "info",
    "message": "Orchestrator started",
    "source": "amos-orchestrator"
  },
  {
    "level": "warn",
    "message": "Disk usage above 80%"
  }
]
```

**Response:** `201 Created` on success. Returns `418` if the device is not registered.

### `GET /v1/logs/devices`

Query historic device log entries, most recent first. All query parameters are optional:

- `device_id` — only entries for this device.
- `level` — minimum severity (`level <= entry.level`), e.g., `level=warn` returns `warn`, `error`, and `fatal` entries.
- `page` / `page_size` — pagination, as on other list endpoints.

#### `GET /logs/applications`

Query historic application log entries, most recent first. Same query parameters as `GET /logs/devices`, plus:

- `application_id` — only entries for this application.

#### `GET /logs/stream`

Server-Sent Events (SSE) stream of incoming logs (both device and application logs), filterable via optional query parameters:

- `device_id` — only events for this device.
- `application_id` — only application-log events for this application.
- `level` — minimum severity (`level <= event.level`).
- `kind` — filter by log kind.

Each event is sent as a `message` event with a JSON payload. This stream is in-memory only: it broadcasts entries as they are inserted by the publish endpoint, with no replay/history. It is intended for "live tail" use cases; use the historic GET endpoints for past logs.

Because this endpoint sits behind the global JWT middleware, a browser's native `EventSource` API (which cannot set request headers) cannot be used directly without a workaround.

## Retention

Both `device_logs` and `application_logs` have a TimescaleDB retention policy that automatically drops chunks (and the data within them) older than **1 year**. This is configured in `m20260615_000001_add_log_retention_policy` via `sea_orm_timescale::migration::add_retention_policy`.

### Log Buffering and Lifecycle

The orchestrator utilizes an in-memory buffering system to queue log entries before dispatching them to the Cloud API. This configuration ensures rapid throughput and protects physical hardware components from write-wear or file corruption during sudden power losses.

#### 1. Buffer Capacities and Thresholds

Log shipping is governed by three primary settings (configurable via `config.toml` or `APP_` environment variables):
- **`log_max_buffer` (Default: 10,000 entries):** The absolute memory threshold allocated for log retention.
- **`log_max_batch` (Default: 256 entries):** The chunk size that triggers an immediate transmission to the cloud endpoint once met.
- **`log_flush_interval_secs` (Default: 60s):** A fallback timer ensuring logs are pushed at least once a minute even if the batch size threshold hasn't been met.

#### 2. Fault Tolerance and Strategy during Outages

* **Network Interruption (Cloud Unreachable):** If the Cloud API becomes unavailable, logs accumulate dynamically in the system RAM. When the total entries exceed `log_max_buffer`, a **First-In, First-Out (FIFO)** strategy is executed: the oldest historical entries are dropped from memory (`.drain()`) to prevent system Out-Of-Memory (OOM) faults.
- **Power Interruptions / Hard Reset:** Because logs are cached strictly inside volatile memory, an unexpected loss of device power results in the immediate loss of all un-flushed log entries. This deliberate architectural choice prevents local log files from corrupting disk sectors during hard resets.

## Configuration

The TimescaleDB connection is configured separately from the main database:

```toml
# config.toml
timescale_database_url = "postgres://app:4M0S@127.0.0.1:5433/amos_timeseries"
```

or via the `APP_TIMESCALE_DATABASE_URL` environment variable. The URL must start with `postgres://` — there is no SQLite fallback, since hypertables are PostgreSQL/TimescaleDB-specific.
