# Plan: Dedicated Device/User API Routes with Access Control

## Overview

Separate the existing 55 API endpoints into device-only, user-only, and shared routes. Add middleware-based authorization so devices can only access device endpoints (and only their own data), and users can only access user endpoints.

## Architecture Decision: Layered Middleware

Use Axum's `map_request_with_state` middleware to enforce role-based access per route group. The existing `jwt_auth` middleware stays untouched — a new `require_role` middleware runs after it.

```
Request → jwt_auth (validates token, inserts claims) → require_role (checks role) → handler
```

All existing URL paths stay the same (`/v1/...`). No client-side URL changes needed.

## Endpoint Classification

### Device-only (10 endpoints — orchestrator calls these)

| Method | Path | Reason |
|--------|------|--------|
| `PUT` | `/pings/{device_uuid}` | Device heartbeat |
| `POST` | `/reported-os-assignments` | Report current OS state |
| `POST` | `/reported-app-assignments` | Report current app state |
| `POST` | `/logs/devices` | Send device logs |
| `POST` | `/logs/applications` | Send app logs |
| `GET` | `/catalog` | Available versions |
| `GET` | `/download/*` | Download images |

### User-only (42 endpoints — admin/management CRUD)

All CRUD on: tenants, groups, devices, applications, application_configs, os_versions, os_assignments, application_assignments, reported_os/app_assignments (read), pings (list), logs (read + SSE stream), audit_log.

### Shared (Role::Any — 6 endpoints, both roles can access)

| Method | Path | Reason |
|--------|------|--------|
| `GET` | `/os-assignments` | Devices query by `device_uuid`, users list all |
| `GET` | `/app-assignments` | Same pattern |
| `GET` | `/os-versions/{id}` | Devices need version details, users manage versions |
| `GET` | `/app-configs/{id}` | Devices need config details, users manage configs |
| `GET` | `/reported-os-assignments` | Devices may need to read, users monitor |
| `GET` | `/reported-app-assignments` | Same |

## Device Identity Scoping

When a device accesses an endpoint, the server verifies the device only accesses its own data:

1. Extract `device_uuid` from JWT `sub` claim (already in request extensions as `DeviceClaims`)
2. For endpoints with `?device_uuid=` query param: verify requested UUID matches JWT `sub`
3. For endpoints like `GET /os-versions/{id}`: verify the requesting device has an assignment referencing that resource
4. Return `403 Forbidden` if a device tries to access another device's data

This is implemented as handler-level checks in the relevant device/shared route handlers.

## Subtask Breakdown

### Subtask 1: Authorization Infrastructure

**New file**: `api-mock-server/src/authz.rs`

- `Role` enum: `Device`, `User`, `Any`
- `RequiredRole` state type (wraps `Role`)
- `require_role` middleware: reads claims from request extensions, checks role against requirement, returns 403 on mismatch

**Modified file**: `api-mock-server/src/main.rs`
- Add `mod authz;`

### Subtask 2: Device Routes

**New file**: `api-mock-server/src/api_v1/routes/device_routes.rs`

Router with 10 device-only endpoints + role layer:
```rust
pub fn routes() -> Router {
    Router::new()
        .route("/pings/{device_uuid}", put(pings::upsert_ping))
        .route("/reported-os-assignments", post(reported_os_assignments::create_reported_os_assignment))
        .route("/reported-app-assignments", post(reported_application_assignments::create_reported_application_assignment))
        .route("/logs/devices", post(logs::create_device_logs))
        .route("/logs/applications", post(logs::create_application_logs))
        .route("/catalog", get(catalog_handler))
        .nest_service("/download", ServeDir::new("assets"))
}
```

### Subtask 3: User Routes

**New file**: `api-mock-server/src/api_v1/routes/user_routes.rs`

Router with all remaining CRUD endpoints + role layer:
```rust
pub fn routes() -> Router {
    Router::new()
        .merge(tenants::routes())
        .merge(groups::routes())
        .merge(devices::routes())
        .merge(applications::routes())
        .merge(application_configs::routes())
        .merge(application_assignments::routes())
        .merge(os_versions::routes())
        .merge(os_assignments::routes())
        .merge(reported_os_assignments::routes())
        .merge(reported_application_assignments::routes())
        .merge(pings::routes())           // GET /pings only
        .merge(logs::routes())            // GET endpoints + SSE stream
        .merge(audit_log::routes())
}
```

### Subtask 4: Shared Routes (Role::Any)

**Modified file**: `api-mock-server/src/api_v1/routes/mod.rs`

Assemble all three route groups with their role layers:
```rust
pub fn routes() -> Router {
    let device = device_routes::routes()
        .route_layer(axum::middleware::from_fn_with_state(RequiredRole(Role::Device), require_role));
    let user = user_routes::routes()
        .route_layer(axum::middleware::from_fn_with_state(RequiredRole(Role::User), require_role));
    let shared = shared_routes::routes()
        .route_layer(axum::middleware::from_fn_with_state(RequiredRole(Role::Any), require_role));
    Router::new().merge(device).merge(user).merge(shared)
}
```

**New file**: `api-mock-server/src/api_v1/routes/shared_routes.rs`

Router for endpoints accessible by both roles.

### Subtask 5: Device Identity Scoping

**Modified files**: Route handlers for device-accessible endpoints

For each device-accessible endpoint, add checks:
- `PUT /pings/{device_uuid}`: verify path param matches JWT `sub`
- `POST /reported-os-assignments?device_uuid=`: verify query param matches JWT `sub`
- `POST /reported-app-assignments?device_uuid=`: verify query param matches JWT `sub`
- `POST /logs/devices?device_uuid=`: verify query param matches JWT `sub`
- `POST /logs/applications?device_uuid=`: verify query param matches JWT `sub`
- `GET /os-assignments?device_uuid=`: verify query param matches JWT `sub`
- `GET /app-assignments?device_uuid=`: verify query param matches JWT `sub`
- `GET /os-versions/{id}`: verify requesting device has an assignment referencing this version
- `GET /app-configs/{id}`: verify requesting device has an assignment referencing this config

Return `403 Forbidden` if scoping check fails.

### Subtask 6: Route Conflict Resolution

Some paths exist in multiple route groups with different HTTP methods:

| Path | Device | User | Resolution |
|------|--------|------|------------|
| `/logs/devices` | POST | GET | Different methods — Axum handles this fine |
| `/logs/applications` | POST | GET | Same |
| `/reported-os-assignments` | POST | GET | POST in device_routes, GET in user_routes |
| `/reported-app-assignments` | POST | GET | Same |

For GET-only shared endpoints (`/os-assignments`, `/app-assignments`, `/os-versions/{id}`, `/app-configs/{id}`), use `Role::Any` in shared_routes.

### Subtask 7: Testing

**Modified file**: `api-mock-server/src/api_v1/routes/mod.rs` (tests section)

- Add `test_app_with_device_auth()` helper: builds full middleware stack with valid device JWT
- Add `test_app_with_user_auth()` helper: builds full middleware stack with valid user JWT
- Integration test matrix:
  - Device token → device endpoint = 200
  - Device token → user endpoint = 403
  - Device token → shared endpoint = 200
  - User token → user endpoint = 200
  - User token → device endpoint = 403
  - User token → shared endpoint = 200
  - No token → 401
- Device scoping tests:
  - Device A accessing Device A's data = 200
  - Device A accessing Device B's data = 403

Existing tests (which bypass auth via `oneshot`) continue to pass unchanged.

## Files to Create/Modify

| File | Action |
|------|--------|
| `api-mock-server/src/authz.rs` | **Create** — Role enum, require_role middleware |
| `api-mock-server/src/api_v1/routes/device_routes.rs` | **Create** — Device-only router |
| `api-mock-server/src/api_v1/routes/user_routes.rs` | **Create** — User-only router |
| `api-mock-server/src/api_v1/routes/shared_routes.rs` | **Create** — Shared router (Role::Any) |
| `api-mock-server/src/api_v1/routes/mod.rs` | **Modify** — Reassemble with role layers |
| `api-mock-server/src/main.rs` | **Modify** — Add `mod authz;`, move /catalog and /download |
| `api-mock-server/src/api_v1/routes/pings.rs` | **Modify** — Split device PUT from user GET |
| `api-mock-server/src/api_v1/routes/logs.rs` | **Modify** — Split device POST from user GET |
| `api-mock-server/src/api_v1/routes/reported_os_assignments.rs` | **Modify** — Add device scoping to POST handler |
| `api-mock-server/src/api_v1/routes/reported_application_assignments.rs` | **Modify** — Add device scoping to POST handler |
| `api-mock-server/src/api_v1/routes/os_assignments.rs` | **Modify** — Add device scoping to GET handler |
| `api-mock-server/src/api_v1/routes/application_assignments.rs` | **Modify** — Add device scoping to GET handler |
| `api-mock-server/src/api_v1/routes/os_versions.rs` | **Modify** — Add device ownership check to GET by id |
| `api-mock-server/src/api_v1/routes/application_configs.rs` | **Modify** — Add device ownership check to GET by id |

## No Changes Needed

- `orchestrator/src/api_client.rs` — already uses correct endpoints with device tokens
- `api-mock-server/src/middleware.rs` — jwt_auth stays unchanged
- `api-mock-server/src/auth_device.rs` — no changes
- `api-mock-server/src/auth_user.rs` — no changes
- Shell E2E tests — use user tokens, no changes needed

## Verification

1. `cargo test` — all existing + new tests pass
2. `cargo clippy` — no warnings
3. Manual: start server, send device token to user endpoint → 403
4. Manual: send user token to device endpoint → 403
5. Manual: device A token to device B's data → 403
6. Shell E2E scripts still pass
