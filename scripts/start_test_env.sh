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

echo -e "${BLUE}Starting Zero-Downtime Update Test Environment...${NC}"

# 1. Check for Docker or Podman
COMPOSE_CMD=""
if command -v docker &> /dev/null && docker compose version &> /dev/null; then
    COMPOSE_CMD="docker compose"
elif command -v podman &> /dev/null && podman compose version &> /dev/null; then
    COMPOSE_CMD="podman compose"
else
    echo -e "${RED}Error: Neither 'docker compose' nor 'podman compose' found.${NC}"
    exit 1
fi

# 2. Check for Lima (required for the Edge IPC VM)
if ! command -v limactl &> /dev/null; then
    echo -e "${RED}Error: 'limactl' is not installed.${NC}"
    echo "Please install Lima to run the Edge VM:"
    echo "  macOS: brew install lima"
    echo "  Linux: sudo apt install lima (or equivalent)"
    exit 1
fi

# 3. Start Backend & Database via Compose
echo -e "${BLUE}Starting API Mock Cloud and Database...${NC}"
$COMPOSE_CMD -f .github/workflows/test-env-compose.yml up -d

# Wait a few seconds to let the database and Rust migrations settle
echo "Waiting for services to initialize..."
sleep 3

# 4. Start Edge VM via Lima
echo -e "${BLUE}Starting Edge IPC VM (Lima)...${NC}"
limactl start .github/workflows/disk-image.yaml

# 5. Success Output
echo -e "${GREEN}====================================================${NC}"
echo -e "${GREEN}✅ Test Environment is running successfully!${NC}"
echo -e "The database was automatically initialized via Rust migrations."
echo -e "Mock Cloud API is reachable at: http://localhost:8080"
echo -e " "
echo -e "=> Log into your Edge IPC by running: ${GREEN}lima shell edge-vm${NC}"
echo -e "${GREEN}====================================================${NC}"