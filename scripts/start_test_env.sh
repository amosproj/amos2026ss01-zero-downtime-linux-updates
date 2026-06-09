#!/usr/bin/env bash

# Exit immediately if a command exits with a non-zero status
set -e

# Define colors for terminal output
GREEN='\033[0;32m'
RED='\033[0;31m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Ensure we run commands relative to the repository root
# This allows running the script from anywhere (e.g., inside /scripts or root)
REPO_ROOT=$(git rev-parse --show-toplevel 2>/dev/null || pwd)
cd "$REPO_ROOT"

# Optional release tag for the Edge IPC disk image.
# If not provided, Lima will use the local dist/ build.
# Example: ./scripts/start_test_env.sh test-setup-01
TAG=${1:-}

echo -e "${BLUE}Starting Zero-Downtime Update Test Environment...${NC}"

# 1. Check for Docker or Podman (required for the PostgreSQL container)
COMPOSE_CMD=""
if command -v docker &> /dev/null && docker compose version &> /dev/null; then
    COMPOSE_CMD="docker compose"
elif command -v podman &> /dev/null && podman compose version &> /dev/null; then
    COMPOSE_CMD="podman compose"
else
    echo -e "${RED}Error: Neither 'docker compose' nor 'podman compose' found.${NC}"
    exit 1
fi

# 2. Check for Cargo (required to run the API Mock Server)
if ! command -v cargo &> /dev/null; then
    echo -e "${RED}Error: 'cargo' is not installed.${NC}"
    echo "Please install Rust: https://rustup.rs"
    exit 1
fi

# 3. Check for Lima (required for the Edge IPC VM)
if ! command -v limactl &> /dev/null; then
    echo -e "${RED}Error: 'limactl' is not installed.${NC}"
    echo "Please install Lima to run the Edge VM:"
    echo "  macOS: brew install lima"
    echo "  Linux: sudo apt install lima (or equivalent)"
    exit 1
fi

# 4. Start PostgreSQL via Compose
echo -e "${BLUE}Starting PostgreSQL database...${NC}"
$COMPOSE_CMD -f .devcontainer/docker-compose.yml up -d postgres-container

echo "Waiting for PostgreSQL to become ready..."
sleep 4

# 5. Build and start the API Mock Server directly on the host
# Database credentials must match the defaults in api-mock-server/src/config.rs
# and the PostgreSQL container
echo -e "${BLUE}Building and starting API Mock Server (cargo run)...${NC}"
APP_DATABASE_URL="postgres://app:4M0S@127.0.0.1:5432/amos" \
  cargo run --package amos-api-mock-server \
  > /tmp/amos-api-mock.log 2>&1 &
API_PID=$!

# Give the server a moment to start (includes DB migration on first run)
echo "Waiting for API Mock Server to initialize (PID $API_PID)..."
sleep 5

# Verify the server process is still alive
if ! kill -0 "$API_PID" 2>/dev/null; then
    echo -e "${RED}Error: API Mock Server failed to start. Check /tmp/amos-api-mock.log${NC}"
    cat /tmp/amos-api-mock.log
    exit 1
fi

# Persist the PID so the server can be stopped later
echo "$API_PID" > /tmp/amos-api-mock.pid

# 6. Start Edge IPC VM via Lima
# If a tag is provided, override the fallback release URLs with the tagged artifacts.
# Otherwise Lima will use the local dist/ build (see dev-env/lima/edge-ipc.yaml).
echo -e "${BLUE}Starting Edge IPC VM (Lima)...${NC}"
LIMA_TEMPLATE="dev-env/lima/edge-ipc.yaml"
GHCR_BASE="ghcr.io/amosproj/amos2026ss01-zero-downtime-linux-updates-disk"
# BASE_URL="https://github.com/amosproj/amos2026ss01-zero-downtime-linux-updates/releases/download"

if [[ -n "$TAG" ]]; then
    echo "Pulling disk image for tag: ${TAG} via ORAS..."

    # Check for oras
    if ! command -v oras &> /dev/null; then
        echo -e "${RED}Error: 'oras' is not installed.${NC}"
        echo "  macOS: brew install oras"
        echo "  Linux: https://oras.land/docs/installation"
        exit 1
    fi

    # Determine arch
    ARCH=$(uname -m)
    case "$ARCH" in
        (x86_64)  LIMA_ARCH="amd64" ;;
        (aarch64|arm64) LIMA_ARCH="arm64" ;;
        (*) echo -e "${RED}Unsupported architecture: $ARCH${NC}"; exit 1 ;;
    esac

    mkdir -p dist/oras
    ARTIFACT="amos-edge-${TAG}-${LIMA_ARCH}.raw"
    ARTIFACT_XZ="${ARTIFACT}.xz"
    if [[ -f "dist/oras/${ARTIFACT}" ]]; then
        echo "Disk image already present, skipping download."
    else
        (cd dist/oras && oras pull "${GHCR_BASE}:${TAG}-${LIMA_ARCH}")
        xz -d "dist/oras/${ARTIFACT_XZ}"
    fi

    

    RAW_PATH="$(pwd)/dist/oras/amos-edge-${TAG}-${LIMA_ARCH}.raw"
    limactl start --name edge-ipc \
        --set ".images = [{\"location\": \"${RAW_PATH}\", \"arch\": \"x86_64\"}, {\"location\": \"${RAW_PATH}\", \"arch\": \"aarch64\"}]" \
        "$LIMA_TEMPLATE"
else
    echo "No tag provided — using local dist/ build. Provide a tag with './scripts/start_test_env.sh test-setup-XX'"
    limactl start --name edge-ipc "$LIMA_TEMPLATE"
fi

# 7. Success Output
echo -e "${GREEN}====================================================${NC}"
echo -e "${GREEN}✅ Test Environment is running successfully!${NC}"
echo -e ""
echo -e "  PostgreSQL:       localhost:5432"
echo -e "  Mock Cloud API:   http://localhost:8080   (log: /tmp/amos-api-mock.log)"
echo -e "  API Mock PID:     $API_PID  (stop with: kill \$(cat /tmp/amos-api-mock.pid))"
echo -e ""
echo -e "  Log into Edge IPC:        ${GREEN}limactl shell edge-ipc${NC}"
echo -e "  Watch orchestrator logs:  ${GREEN}limactl shell edge-ipc -- journalctl -u orchestrator.service -f${NC}"
echo -e ""
echo -e "  Stop everything:"
echo -e "    kill \$(cat /tmp/amos-api-mock.pid)"
echo -e "    $COMPOSE_CMD -f .github/workflows/test-env-compose.yml down"
echo -e "    limactl stop edge-ipc"
echo -e "${GREEN}====================================================${NC}"