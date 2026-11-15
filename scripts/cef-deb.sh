#!/usr/bin/env bash
# Builds the Debian package for the CEF shell.
#
# `tauri build --bundles deb` cannot finish this bundle yet: the alpha bundler
# requires `libEGL.so`/`libGLESv2.so` in the CEF distribution (CEF 154 for Linux ships
# ANGLE as part of `libcef.so` and only Vulkan/SwiftShader beside it) and its `strip`
# invocation fails on `libcef.so`. This script assembles the same layout the bundler
# starts with, and adds what the app needs to start:
#
#   /usr/share/OpenCLion/      binary + libcef.so + icudtl.dat + paks + locales/
#   /usr/bin/openclion         symlink (the binary loads libcef.so from its own dir, $ORIGIN)
#   /usr/lib/openclion/        a copy of chrome-sandbox is not needed: the helper stays next
#                              to libcef.so, and postinst gives it root:root 4755
#
# Usage: scripts/cef-deb.sh [CEF dir] [output dir]
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cef_dir="${1:-/opt/cefnew/cef_linux_x86_64}"
out_dir="${2:-$root/target/release/bundle/deb}"
version="$(python3 -c "import json;print(json.load(open('$root/src-tauri/tauri.conf.json'))['version'])")"
binary="$root/target/release/openclion"

[[ -x "$binary" ]] || { echo "build first: CEF_PATH=... cargo build --release -p openclion" >&2; exit 1; }
[[ -f "$cef_dir/libcef.so" ]] || { echo "no libcef.so in $cef_dir" >&2; exit 1; }

stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT
app_dir="$stage/usr/share/OpenCLion"
install -d "$app_dir" "$stage/usr/bin" "$stage/usr/share/applications" \
  "$stage/usr/share/icons/hicolor/32x32/apps" "$stage/usr/share/icons/hicolor/128x128/apps" \
  "$stage/usr/share/icons/hicolor/256x256/apps" "$stage/DEBIAN"

install -m 0755 "$binary" "$app_dir/openclion"
ln -s ../share/OpenCLion/openclion "$stage/usr/bin/openclion"

# CEF runtime: everything the distribution ships beside libcef.so.
for f in libcef.so libvk_swiftshader.so libvulkan.so.1 vk_swiftshader_icd.json \
         icudtl.dat v8_context_snapshot.bin resources.pak chrome_100_percent.pak \
         chrome_200_percent.pak; do
  [[ -f "$cef_dir/$f" ]] && install -m 0644 "$cef_dir/$f" "$app_dir/$f"
done
if [[ "${CEF_DEB_KEEP_SYMBOLS:-0}" != "1" ]] && [[ -f "$app_dir/libcef.so" ]]; then
  strip --strip-unneeded "$app_dir/libcef.so" 2>/dev/null || echo "warning: strip failed, keeping symbols" >&2
fi
[[ -x "$cef_dir/chrome-sandbox" ]] && install -m 4755 "$cef_dir/chrome-sandbox" "$app_dir/chrome-sandbox"
if [[ -d "$cef_dir/locales" ]]; then
  cp -a "$cef_dir/locales" "$app_dir/locales"
  chmod -R a+rX "$app_dir/locales"
fi

install -m 0644 "$root/src-tauri/icons/32x32.png" "$stage/usr/share/icons/hicolor/32x32/apps/openclion.png"
install -m 0644 "$root/src-tauri/icons/128x128.png" "$stage/usr/share/icons/hicolor/128x128/apps/openclion.png"
install -m 0644 "$root/src-tauri/icons/128x128@2x.png" "$stage/usr/share/icons/hicolor/256x256/apps/openclion.png"

cat > "$stage/usr/share/applications/OpenCLion.desktop" <<'DESKTOP'
[Desktop Entry]
Type=Application
Name=OpenCLion
GenericName=IDE
Comment=An open-source C/C++ IDE in the CLion tradition
Exec=openclion %F
Icon=openclion
Terminal=false
Categories=Development;IDE;
StartupWMClass=openclion
MimeType=inode/directory;
DESKTOP

installed_kb="$(du -sk "$stage" | cut -f1)"
cat > "$stage/DEBIAN/control" <<CONTROL
Package: openclion
Version: $version
Architecture: amd64
Installed-Size: $installed_kb
Maintainer: OpenCLion contributors
Section: devel
Priority: optional
Depends: libgtk-4-1, libglib2.0-0 | libglib2.0-0t64, libnss3, libnspr4, libgbm1, libdrm2, libxkbcommon0, libxcomposite1, libxdamage1, libxrandr2, libx11-6, libxcb1, libexpat1, libdbus-1-3, libatk-bridge2.0-0 | libatk-bridge2.0-0t64, libatspi2.0-0 | libatspi2.0-0t64, libpango-1.0-0, libcairo2, libasound2 | libasound2t64, libcups2 | libcups2t64
Description: An open-source C/C++ IDE in the CLion tradition
 OpenCLion is an open-source C/C++ IDE: CMake projects, clangd code
 insight, LLDB debugging. This build runs the user interface on Chromium
 (CEF) instead of WebKitGTK.
CONTROL

install -m 0755 "$root/src-tauri/linux/postinst.sh" "$stage/DEBIAN/postinst"
install -m 0755 "$root/src-tauri/linux/prerm.sh" "$stage/DEBIAN/prerm" 2>/dev/null || true

mkdir -p "$out_dir"
deb="$out_dir/OpenCLion_${version}_amd64.deb"
dpkg-deb --root-owner-group -Zxz --build "$stage" "$deb" >/dev/null
echo "$deb"
