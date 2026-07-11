#!/usr/bin/env bash
# Reset a demo api-server run: down, drop its volumes, up.
#
# Usage: ./reset-run.sh [run1|run2|run3]  (default: run1)
set -euo pipefail

RUN="${1:-run1}"
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
COMPOSE_FILE="$DIR/$RUN/compose.yaml"

if [ ! -f "$COMPOSE_FILE" ]; then
	echo "error: no such run '$RUN' (expected $COMPOSE_FILE)" >&2
	exit 1
fi

echo ">>> Stopping $RUN"
podman compose -f "$COMPOSE_FILE" down

echo ">>> Removing $RUN volumes"
podman volume rm -f "${RUN}_postgres_data" "${RUN}_timescale_data"

echo ">>> Starting $RUN"
podman compose -f "$COMPOSE_FILE" up -d
