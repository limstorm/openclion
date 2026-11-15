#!/usr/bin/env bash
# Packages the CEF shell: Debian package and/or AppImage.
#
# `tauri build --bundles deb`/`appimage` cannot finish a CEF bundle with the
# 3.0.0-alpha.6 CLI yet: its bundler expects `libEGL.so`/`libGLESv2.so` beside
# `libcef.so`, which CEF 154 no longer ships for Linux (ANGLE lives inside
# `libcef.so`; only SwiftShader and the Vulkan loader sit next to it), so it stops with
# `No such file or directory` while copying the runtime. This script assembles the same
# layouts by hand, with what the app needs to start:
#
#   /usr/share/OpenCLion/      binary + libcef.so + icudtl.dat + paks + locales/
#   /usr/bin/openclion         symlink (the binary loads libcef.so from its own dir, $ORIGIN)
#   chrome-sandbox             root:root 4755 (also re-applied by postinst)
#
# Usage:
#   scripts/cef-package.sh [--bundle deb|appimage|all] [--keep-symbols] [CEF dir]
#
# `--keep-symbols` ships libcef.so with its symbol table (1.45 GiB instead of 272 MiB):
# use it for builds that have to symbolize Chromium crashes.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
bundle="all"
keep_symbols="${CEF_DEB_KEEP_SYMBOLS:-0}"
cef_dir=""
out_dir="$root/target/release/bundle"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --bundle) bundle="$2"; shift 2 ;;
    --keep-symbols) keep_symbols=1; shift ;;
    -h|--help) sed -n '2,20p' "$0"; exit 0 ;;
    *) cef_dir="$1"; shift ;;
  esac
done
cef_dir="${cef_dir:-/opt/cefnew/cef_linux_x86_64}"

version="$(python3 -c "import json;print(json.load(open('$root/src-tauri/tauri.conf.json'))['version'])")"
binary="$root/target/release/openclion"
[[ -x "$binary" ]] || { echo "build first: CEF_PATH=... cargo build --release -p openclion" >&2; exit 1; }
[[ -f "$cef_dir/libcef.so" ]] || { echo "no libcef.so in $cef_dir" >&2; exit 1; }

# Copies the binary and the CEF runtime into $1 (the directory that ends up next to the
# binary in the package).
populate() {
  local dir="$1"
  install -d "$dir"
  install -m 0755 "$binary" "$dir/openclion"
  local f
  for f in libcef.so libvk_swiftshader.so libvulkan.so.1 vk_swiftshader_icd.json \
           icudtl.dat v8_context_snapshot.bin resources.pak chrome_100_percent.pak \
           chrome_200_percent.pak; do
    [[ -f "$cef_dir/$f" ]] && install -m 0644 "$cef_dir/$f" "$dir/$f"
  done
  [[ -x "$cef_dir/chrome-sandbox" ]] && install -m 4755 "$cef_dir/chrome-sandbox" "$dir/chrome-sandbox"
  if [[ -d "$cef_dir/locales" ]]; then
    cp -a "$cef_dir/locales" "$dir/locales"
    chmod -R a+rX "$dir/locales"
  fi
  if [[ "$keep_symbols" != "1" && -f "$dir/libcef.so" ]]; then
    strip --strip-unneeded "$dir/libcef.so" 2>/dev/null || echo "warning: strip failed, keeping symbols" >&2
  fi
}

desktop_entry() {
  cat <<'DESKTOP'
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
}

build_deb() {
  local stage app_dir installed_kb deb
  stage="$(mktemp -d)"
  app_dir="$stage/usr/share/OpenCLion"
  install -d "$app_dir" "$stage/usr/bin" "$stage/usr/share/applications" \
    "$stage/usr/share/icons/hicolor/32x32/apps" "$stage/usr/share/icons/hicolor/128x128/apps" \
    "$stage/usr/share/icons/hicolor/256x256/apps" "$stage/DEBIAN"
  populate "$app_dir"
  ln -s ../share/OpenCLion/openclion "$stage/usr/bin/openclion"
  install -m 0644 "$root/src-tauri/icons/32x32.png" "$stage/usr/share/icons/hicolor/32x32/apps/openclion.png"
  install -m 0644 "$root/src-tauri/icons/128x128.png" "$stage/usr/share/icons/hicolor/128x128/apps/openclion.png"
  install -m 0644 "$root/src-tauri/icons/128x128@2x.png" "$stage/usr/share/icons/hicolor/256x256/apps/openclion.png"
  desktop_entry > "$stage/usr/share/applications/OpenCLion.desktop"

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
  install -m 0755 "$root/src-tauri/linux/prerm.sh" "$stage/DEBIAN/prerm"

  install -d "$out_dir/deb"
  local suffix=""
  [[ "$keep_symbols" == "1" ]] && suffix="-symbols"
  deb="$out_dir/deb/OpenCLion_${version}_amd64${suffix}.deb"
  dpkg-deb --root-owner-group -Zxz --build "$stage" "$deb" >/dev/null
  rm -rf "$stage"
  echo "$deb"
}

build_appimage() {
  local tool="${APPIMAGETOOL:-/opt/appimage/appimagetool}"
  [[ -x "$tool" ]] || { echo "no appimagetool at $tool (set APPIMAGETOOL)" >&2; return 1; }
  local dir="$out_dir/appimage/OpenCLion.AppDir"
  rm -rf "$dir"
  install -d "$dir"
  # Everything sits next to the binary: it links with -rpath,$ORIGIN.
  populate "$dir/usr/bin"
  desktop_entry > "$dir/openclion.desktop"
  install -m 0644 "$root/src-tauri/icons/128x128@2x.png" "$dir/openclion.png"
  cat > "$dir/AppRun" <<'APPRUN'
#!/bin/sh
# AppImage entry point: the binary finds libcef.so and the paks next to itself.
here="$(dirname "$(readlink -f "$0")")"
exec "$here/usr/bin/openclion" "$@"
APPRUN
  chmod +x "$dir/AppRun"

  install -d "$out_dir/appimage"
  local image="$out_dir/appimage/OpenCLion_${version}_amd64.AppImage"
  ARCH=x86_64 "$tool" --appimage-extract-and-run "$dir" "$image" >/dev/null
  chmod +x "$image"
  echo "$image"
}

case "$bundle" in
  deb) build_deb ;;
  appimage) build_appimage ;;
  all) build_deb; build_appimage ;;
  *) echo "unknown bundle: $bundle (deb|appimage|all)" >&2; exit 1 ;;
esac
