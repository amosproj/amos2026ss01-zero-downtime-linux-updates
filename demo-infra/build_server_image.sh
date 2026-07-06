#!/bin/bash
set -eu

binary_url="$1"
binary_path="api-server"

cleanup() {
    rm -f "$binary_path"
}

trap cleanup EXIT

curl -sSL -o "${binary_path}" "$binary_url"

podman build -t "localhost/api-server" -f "files/Containerfile" --build-arg "API_BINARY_PATH=${binary_path}" .
