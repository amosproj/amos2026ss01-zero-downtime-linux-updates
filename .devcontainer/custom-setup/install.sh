#!/usr/bin/env bash

# IMPORTANT
#
# Please update lastChanged in devcontainer.json when changing this
# file so everyone gets prompted to rebuild the container

set -e

# Bring in ID, ID_LIKE, VERSION_ID, VERSION_CODENAME
. /etc/os-release

if [ "${ID}" != "ubuntu" ]; then
    echo "Linux distro ${ID} not supported"
    exit 1
fi

export DEBIAN_FRONTEND=noninteractive

# This script runs when the container gets built
# Use it to install additional packages from apt or do some setup, e.g.
# apt -y install --no-install-recommends curl