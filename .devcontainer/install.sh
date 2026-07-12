#!/usr/bin/env bash

# !!! IMPORTANT !!!
#
# Please update installScriptVersion at the end of devcontainer.json when
# changing this file so everyone gets prompted to rebuild the container

set -e

if [ "$(id -u)" != 0 ]; then
    echo -e "Script must be run as root user"
    exit 1
fi

# Bring in ID, ID_LIKE, VERSION_ID, VERSION_CODENAME
. /etc/os-release

if [ "${ID}" != "ubuntu" ]; then
    echo -e "Linux distro ${ID} not supported"
    exit 1
fi

export DEBIAN_FRONTEND=noninteractive

# This script runs when the container gets built
# Use it e.g. to install additional packages from apt:
#
apt-get update -y


apt-get update -y
# Podman CLI for interacting with the socket from podman-container
apt-get install -y --no-install-recommends podman
# for TPM stuff
apt-get install -y --no-install-recommends libtss2-dev
apt-get install -y --no-install-recommends pkg-config
rm -rf /var/lib/apt/lists/*
