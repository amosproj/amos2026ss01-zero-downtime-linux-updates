#!/bin/sh
set -eu

[ -f /etc/amos-dev-image ] || exit 0

if bootc status --json 2>/dev/null | grep -q '"image": *"ghcr.io/amosproj/'; then
    echo "dev image booted against prod update target — refusing boot" >&2
    exit 1
fi

exit 0
