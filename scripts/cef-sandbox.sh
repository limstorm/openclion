#!/usr/bin/env bash
# Chromium's setuid sandbox helper: every build copies a fresh `chrome-sandbox` next to the
# binary without the setuid bit, and CEF aborts at start-up unless it is root-owned 4755.
# Usage: scripts/cef-sandbox.sh [target-dir]   (default: target/debug)
set -euo pipefail
dir="${1:-target/debug}"
helper="$dir/chrome-sandbox"
if [[ ! -f "$helper" ]]; then
  echo "no $helper: build first (CEF_PATH=... cargo build -p openclion)" >&2
  exit 1
fi
sudo chown root:root "$helper"
sudo chmod 4755 "$helper"
ls -l "$helper"
