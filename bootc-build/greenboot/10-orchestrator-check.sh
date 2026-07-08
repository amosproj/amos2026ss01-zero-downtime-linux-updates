#!/bin/sh
dev_binary="/var/usrlocal/bin/amos-orchestrator"
if [ -x "$dev_binary" ]; then
    echo "Using dev binary for bootc check"
    "$dev_binary" -s
else
    /usr/libexec/amos-orchestrator -s
fi
