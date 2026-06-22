#!/usr/bin/env bash
# Flash an installer ISO to a USB stick. DESTRUCTIVE — wipes the target device.
#
# Usage: sudo ./scripts/flash-usb.sh <iso> <device>
#   e.g. sudo ./scripts/flash-usb.sh dist/bootiso/install.iso /dev/sdX
#
# Find the device first:  lsblk -dpno NAME,SIZE,MODEL,TRAN
set -euo pipefail

ISO="${1:-}"
DEV="${2:-}"

die() { echo "error: $*" >&2; exit 1; }

[ -n "$ISO" ] && [ -n "$DEV" ] || die "usage: $0 <iso> <device>"
[ -f "$ISO" ] || die "ISO not found: $ISO"
[ -b "$DEV" ] || die "not a block device: $DEV"
[ "$(id -u)" -eq 0 ] || die "must run as root (use sudo)"

# Refuse anything that isn't clearly a whole disk, and never a mounted root.
case "$DEV" in
  /dev/sda|/dev/nvme0n1|/dev/vda) die "refusing to write to likely system disk $DEV" ;;
esac
if lsblk -no MOUNTPOINT "$DEV" | grep -q '^/$'; then
  die "$DEV holds the root filesystem — refusing"
fi

echo "About to OVERWRITE $DEV with $ISO"
lsblk -dpno NAME,SIZE,MODEL,TRAN "$DEV" || true
read -r -p "Type the device path again to confirm: " CONFIRM
[ "$CONFIRM" = "$DEV" ] || die "confirmation mismatch, aborting"

# Unmount any mounted partitions of the target.
for part in $(lsblk -nlpo NAME "$DEV" | tail -n +2); do
  umount "$part" 2>/dev/null || true
done

echo "Writing... (this can take a few minutes)"
dd if="$ISO" of="$DEV" bs=4M status=progress oflag=direct conv=fsync
sync
echo "Done. You can now boot the IPC from $DEV."
