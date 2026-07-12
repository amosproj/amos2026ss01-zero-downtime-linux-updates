#!/usr/bin/env bash
# Verifies the orchestrator's aliveness pings carry a plausible system uptime

set -euo pipefail

cd "$(dirname "$0")"
source ./common_env.sh

echo "=== Testing Device Ping Uptime ==="

# Restart so the orchestrator's ping loop fires its immediate first tick now,
# instead of waiting up to 60s for the next scheduled ping.
limactl shell "${VM_NAME}" -- sudo systemctl restart orchestrator.service

echo "Polling /v1/pings for device 1's uptime_secs..."
UPTIME=""
for i in $(seq 1 30); do
    PINGS=$(curl -sS -H "Authorization: Bearer ${JWT}" "${HOST_SERVER_URL}/v1/pings")
    UPTIME=$(echo "${PINGS}" | jq -r '.data[] | select(.device_id==1) | .uptime_secs')
    if [ -n "${UPTIME}" ] && [ "${UPTIME}" != "null" ]; then
        break
    fi
    echo "  [${i}/30] No ping with uptime yet, retrying in 3s..."
    sleep 3
done

if [ -z "${UPTIME}" ] || [ "${UPTIME}" == "null" ]; then
    echo -e "${RED}Failure: device 1 reported no uptime_secs after 90s.${NC}"
    echo "Full pings payload: ${PINGS}"
    exit 1
fi

# /proc/uptime is a positive integer of seconds since boot.
if ! [[ "${UPTIME}" =~ ^[0-9]+$ ]] || [ "${UPTIME}" -le 0 ]; then
    echo -e "${RED}Failure: uptime_secs is not a positive integer: '${UPTIME}'${NC}"
    exit 1
fi

echo -e "${GREEN}Success: device 1 reported uptime_secs=${UPTIME}.${NC}"
