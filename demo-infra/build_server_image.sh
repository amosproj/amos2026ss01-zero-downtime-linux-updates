#!/bin/bash
set -eu

binary_path="api-server"

cp ~/amos-api-mock-server "$binary_path"

podman build -t "localhost/api-server" -f "files/Containerfile" --build-arg "API_BINARY_PATH=${binary_path}" .
