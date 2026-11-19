#!/usr/bin/env bash
# Launches the native (nsh) OpenCLion build with the local GTK sysroot.
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export GDK_BACKEND="${GDK_BACKEND:-x11}"
export GTK_IM_MODULE="${GTK_IM_MODULE:-gtk-im-context-simple}"
export LD_LIBRARY_PATH="$DIR/../.sysroot/usr/lib/x86_64-linux-gnu:${LD_LIBRARY_PATH:-}"
cd "$DIR"
exec "$DIR/target/debug/openclion-native" "$@"
