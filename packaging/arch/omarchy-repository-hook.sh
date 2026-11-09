#!/bin/bash
# Omarchy runs this hook before replacing packages after a pacman configuration refresh.
set -euo pipefail
sudo /usr/lib/sonora-fork/enable-repository
