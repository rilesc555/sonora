#!/bin/bash
# Restore only this package's repository include after an Omarchy pacman config refresh.
set -euo pipefail
if (( EUID != 0 )); then
  echo 'run this helper as root' >&2
  exit 1
fi
test -f /etc/pacman.d/sonora-rilesc555.conf
include='Include = /etc/pacman.d/sonora-rilesc555.conf'
if ! grep -Fxq "$include" /etc/pacman.conf; then
  printf '\n# Sonora fork repository, installed by sonora-rilesc555-bin\n%s\n' "$include" >> /etc/pacman.conf
fi
