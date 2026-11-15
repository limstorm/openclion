#!/usr/bin/env bash
# Makes `tauri build --bundles deb|appimage` work for the CEF runtime.
#
# The 3.0.0-alpha.6 bundler copies a fixed list of files out of the CEF distribution and
# fails with `No such file or directory` when one is missing. Two entries of that list,
# `libEGL.so` and `libGLESv2.so`, are ANGLE libraries that CEF stopped shipping for Linux
# by 154 (ANGLE is inside `libcef.so`; only SwiftShader and the Vulkan loader sit beside
# it). Presence is all the bundler checks, and Chromium never opens them on this
# platform, so the shim creates two empty files with that name — an invalid ELF that a
# `dlopen` would reject, exactly like their absence, but enough for the copy loop.
#
# Usage: scripts/cef-bundle-prep.sh [CEF dir]
set -euo pipefail
cef_dir="${1:-${CEF_PATH:-/opt/cef}/154.0.34/cef_linux_x86_64}"
[[ -f "$cef_dir/libcef.so" ]] || { echo "no libcef.so in $cef_dir" >&2; exit 1; }
for shim in libEGL.so libGLESv2.so; do
  if [[ ! -e "$cef_dir/$shim" ]]; then
    : > "$cef_dir/$shim"
    echo "created bundler shim $cef_dir/$shim"
  fi
done
