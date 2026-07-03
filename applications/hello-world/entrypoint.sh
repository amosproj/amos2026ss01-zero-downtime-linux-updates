#!/bin/sh
# stdout → LogLevel::Info, stderr → LogLevel::Error (log_registry.rs:15).
set -e

HEARTBEAT_INTERVAL="${HEARTBEAT_INTERVAL:-5}"
beat=0

# ── helpers ──────────────────────────────────────────────────────────────────

hr() { printf '%.0s─' $(seq 1 60); printf '\n'; }

print_banner() {
    hr
    printf '  🦊  AMOS Hello World  │  v%s\n' "${APP_VERSION:-dev}"
    printf '  hostname : %s\n' "$(hostname)"
    printf '  started  : %s\n' "$(date -u '+%Y-%m-%dT%H:%M:%SZ')"
    hr

    # Greet by name if provided
    if [ -n "${NAME:-}" ]; then
        printf '  %s, %s!\n' "${GREETING:-Hello}" "${NAME}"
        hr
    fi

    # Pretty-print every env var, sorted, aligned
    printf '  Environment variables:\n'
    env | sort | while IFS='=' read -r key val; do
        printf '    %-30s = %s\n' "${key}" "${val}"
    done
    hr
}

cleanup() {
    echo "[INFO] $(date -u '+%Y-%m-%dT%H:%M:%SZ')  received shutdown signal — exiting cleanly"
    exit 0
}

# ── main ─────────────────────────────────────────────────────────────────────

trap cleanup TERM INT

print_banner

while true; do
    beat=$((beat + 1))
    ts="$(date -u '+%Y-%m-%dT%H:%M:%SZ')"

    echo "[INFO] ${ts}  heartbeat #${beat}  (app=${APP_VERSION:-dev}  host=$(hostname))"

    # Every 6th beat emit a WARN to stderr so LogLevel::Error is exercised too
    if [ $((beat % 6)) -eq 0 ]; then
        echo "[WARN] ${ts}  periodic stderr probe — beat #${beat}" >&2
    fi

    sleep "${HEARTBEAT_INTERVAL}" &
    wait $!   # wait in background so the trap fires promptly
done
