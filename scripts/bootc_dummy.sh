#!/bin/sh

# This script serves as a mock for the real bootc binary. Can be used for local development where the real bootc behaviour isn't needed at the moment.
# For example, mounting into the bootc build: `podman run --rm -it -v "./scripts/bootc_dummy.sh:/usr/local/bin/bootc" amos:latest`

cat <<EOF
{"apiVersion":"org.containers.bootc/v1","kind":"BootcHost","metadata":{"name":"host"},"spec":{"bootOrder":"default","image":null},"status":{"booted":{"ostree":{"checksum":"123456"},"image":null},"rollback":null,"rollbackQueued":false,"staged":null,"type":null,"usrOverlay":null}}
EOF
