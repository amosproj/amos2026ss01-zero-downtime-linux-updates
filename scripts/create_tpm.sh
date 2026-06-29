#!/bin/bash

# This script initializes the software TPM the same way as our reference hardware is preconfigured.
# As the first argument, the tmp state directory can be provided (default: `/tmp/emulated_tpm`)
# For running this, the packages `swtpm` and `swtpm_setup` (Ubuntu/Fedora) are needed on the host.

set -eu

readonly tpm_state_dir="${1:-/tmp/emulated_tpm}"
readonly tmp_dir="/tmp/swtpm_bootstrap"

mkdir -p "${tmp_dir}/{config,data,cache}"
export XDG_CACHE_HOME="${tmp_dir}/cache"
export XDG_CONFIG_HOME="${tmp_dir}/config"
export XDG_DATA_HOME="${tmp_dir}/data"

# Needs package swtpm-tools (Ubuntu)
/usr/share/swtpm/swtpm-create-user-config-files --skip-if-exist

mkdir -p "$tpm_state_dir"
swtpm_setup --tpm2 --tpmstate "$tpm_state_dir" --createek --create-ek-cert --lock-nvram
