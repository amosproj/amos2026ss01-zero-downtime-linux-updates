#!/usr/bin/env bash
# Validates hello-world application deployment, lifecycle, and teardown

set -euo pipefail

cd "$(dirname "$0")"
source ./common_env.sh

echo "=== Testing Application Deployment ==="
IMAGE="ghcr.io/amosproj/amos2026ss01-zero-downtime-linux-updates-hello-world:0.1.0"

api "/v1/applications"    POST '{ "name": "hello-world", "description": "E2E heartbeat test app" }' 201
api "/v1/app-configs"     POST "{\"device_id\": 1, \"application_id\": 1, \"image\": \"${IMAGE}\", \"config\": {\"environment\": {\"NAME\": \"AMOS\"}}}" 201

ASSIGNMENT=$(curl -sS -X POST -H "Content-Type: application/json" -H "Authorization: Bearer ${JWT}" \
    -d '{ "application_config_id": 1, "device_id": 1 }' \
    "${HOST_SERVER_URL}/v1/app-assignments")
ASSIGNMENT_ID=$(echo "${ASSIGNMENT}" | jq -r '.id')

limactl shell "${VM_NAME}" -- sudo systemctl restart orchestrator.service

echo "Polling for reported application assignment (allows time for GHCR image pull)..."
REPORTED_CFG_ID=""
for i in $(seq 1 60); do
    REPORTED=$(curl -sS -H "Authorization: Bearer ${JWT}" \
        "${HOST_SERVER_URL}/v1/reported-app-assignments?device_id=1")
    REPORTED_CFG_ID=$(echo "${REPORTED}" | jq '.data[0].application_config_id // empty')
    if [ "${REPORTED_CFG_ID}" = "1" ]; then
        break
    fi
    echo "  [${i}/60] Not yet reported, retrying in 3s..."
    sleep 3
done

echo "API server reports active device application_config_id is: '${REPORTED_CFG_ID}'"

if [ "${REPORTED_CFG_ID}" = "1" ]; then
    echo -e "${GREEN}Success: Device correctly deployed and reported hello-world (application_config_id=1).${NC}"
else
    echo -e "${RED}Failure: Device did not report application_config_id=1. Received: '${REPORTED_CFG_ID}'${NC}"
    echo "Full response payload: ${REPORTED}"
    exit 1
fi

echo "Polling application logs for env var NAME=AMOS printed by hello-world..."
ENV_VAR_FOUND=false
APP_LOGS=""
for i in $(seq 1 60); do
    APP_LOGS=$(curl -sS -H "Authorization: Bearer ${JWT}" \
        "${HOST_SERVER_URL}/v1/logs/applications?device_id=1&application_id=1")
    if echo "${APP_LOGS}" | jq -r '.data[].message' | grep -Eq 'NAME[[:space:]]*=[[:space:]]*AMOS'; then
        ENV_VAR_FOUND=true
        break
    fi
    echo "  [${i}/60] Env var not yet in logs, retrying in 3s..."
    sleep 3
done

if [ "${ENV_VAR_FOUND}" = "true" ]; then
    echo -e "${GREEN}Success: Application log confirms NAME=AMOS was injected into the container environment.${NC}"
else
    echo -e "${RED}Failure: NAME=AMOS not found in application logs after 180s.${NC}"
    echo "Full logs payload: ${APP_LOGS}"
    exit 1
fi

echo "=== Testing Application Teardown ==="

echo "Removing app-assignment (id=${ASSIGNMENT_ID}) to trigger orchestrator teardown..."
api "/v1/app-assignments/${ASSIGNMENT_ID}" DELETE '' 204

echo "Waiting for orchestrator to stop and destroy the container..."
for i in $(seq 1 30); do
    CONTAINER_COUNT=$(limactl shell "${VM_NAME}" -- sudo podman ps -a -q -f "label=org.amos.application_id=1" | wc -l)

    if [ "${CONTAINER_COUNT}" -eq 0 ]; then
        echo -e "${GREEN}Success: Container was successfully stopped and destroyed.${NC}"
        exit 0
    fi

    echo "  [${i}/30] Container still exists, waiting..."
    sleep 3
done

echo -e "${RED}Failure: Container was not destroyed within 90s.${NC}"
limactl shell "${VM_NAME}" -- sudo podman ps -a
exit 1
