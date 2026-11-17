#!/usr/bin/env bash
# Fetch and cache the CEF binary distribution this branch links against.
#
#   scripts/cef-sdk.sh                 # download (resumable) into the cache
#   scripts/cef-sdk.sh --check         # verify the cache, download only if missing
#   scripts/cef-sdk.sh --from-file F   # seed from an archive on disk (USB, another machine)
#   scripts/cef-sdk.sh --parts "U1 U2" # concatenate archive parts (e.g. Gitee release assets)
#
# Cache: ${OPENCLION_CEF_CACHE:-$HOME/.cache/openclion/cef}/154.0.34/cef_linux_x86_64
# Then:  export CEF_PATH="${OPENCLION_CEF_CACHE:-$HOME/.cache/openclion/cef}"
set -euo pipefail

VERSION=154.0.34
SHA1=15c91ad767097217b2e82ba8b933fe30b134be69
ARCHIVE="cef_binary_${VERSION}+g14c5a08+chromium-154.0.8037.98_linux64_minimal.tar.bz2"
DEFAULT_URL="https://cef-builds.spotifycdn.com/cef_binary_154.0.34%2Bg14c5a08%2Bchromium-154.0.8037.98_linux64_minimal.tar.bz2"
CACHE="${OPENCLION_CEF_CACHE:-$HOME/.cache/openclion/cef}"
TARGET="$CACHE/$VERSION/cef_linux_x86_64"
FROM_FILE=""
CHECK_ONLY=0
PARTS="${OPENCLION_CEF_PARTS:-}"

die() { echo "cef-sdk: $*" >&2; exit 1; }

while [ $# -gt 0 ]; do
  case "$1" in
    --check) CHECK_ONLY=1 ;;
    --from-file) shift; FROM_FILE="${1:?--from-file needs a path}" ;;
    --parts) shift; PARTS="${1:?--parts needs a space-separated list}" ;;
    -h|--help) sed -n '2,12p' "$0"; exit 0 ;;
    *) die "unknown argument: $1" ;;
  esac
  shift
done

archive_ok() {
  [ -f "$CACHE/$ARCHIVE" ] && [ "$(sha1sum "$CACHE/$ARCHIVE" | cut -d' ' -f1)" = "$SHA1" ]
}

layout_ok() {
  [ -f "$TARGET/archive.json" ] \
    && [ -f "$TARGET/include/cef_app.h" ] \
    && [ -f "$TARGET/libcef.so" ] \
    && [ -d "$TARGET/libcef_dll" ] \
    && grep -q "$VERSION" "$TARGET/archive.json"
}

report() {
  echo "cef-sdk: SDK ready at $TARGET"
  echo "cef-sdk: export CEF_PATH=\"$CACHE\""
}

if layout_ok; then
  report
  exit 0
fi

if [ "$CHECK_ONLY" = 1 ] && ! archive_ok && [ -z "$FROM_FILE" ] && [ -z "$PARTS" ]; then
  die "not cached and --check was given; run without --check to download"
fi

mkdir -p "$CACHE"

if [ ! -f "$CACHE/$ARCHIVE" ]; then
  if [ -n "$FROM_FILE" ]; then
    echo "cef-sdk: copying $FROM_FILE"
    cp --reflink=auto "$FROM_FILE" "$CACHE/$ARCHIVE"
  elif [ -n "$PARTS" ]; then
    tmp="$CACHE/.parts.$$"
    mkdir -p "$tmp"
    n=0
    for url in $PARTS; do
      n=$((n + 1))
      out="$tmp/part$(printf '%02d' "$n")"
      echo "cef-sdk: downloading part $n: $url"
      curl -fL --retry 5 --retry-delay 2 --connect-timeout 20 --max-time 1800 -C - -o "$out" "$url" \
        || die "part $n failed: $url"
    done
    cat "$tmp"/part* > "$CACHE/$ARCHIVE"
    rm -rf "$tmp"
  else
    echo "cef-sdk: downloading $ARCHIVE (resumable, ~311 MiB)"
    ok=0
    for _ in $(seq 1 30); do
      before=$(stat -c%s "$CACHE/$ARCHIVE" 2>/dev/null || echo 0)
      if curl -fL --retry 3 --retry-delay 2 --connect-timeout 20 --max-time 1800 -C - \
          -o "$CACHE/$ARCHIVE" "$DEFAULT_URL"; then ok=1; break; fi
      after=$(stat -c%s "$CACHE/$ARCHIVE" 2>/dev/null || echo 0)
      echo "cef-sdk: transfer interrupted at $after bytes, resuming"
      if [ "$after" -le "$before" ]; then sleep 3; fi
    done
    [ "$ok" = 1 ] || die "download failed; retry later, or pass --from-file/--parts"
  fi
fi

echo "cef-sdk: verifying sha1"
[ "$(sha1sum "$CACHE/$ARCHIVE" | cut -d' ' -f1)" = "$SHA1" ] || die "sha1 mismatch; remove $CACHE/$ARCHIVE and retry"

echo "cef-sdk: extracting"
rm -rf "$CACHE/$VERSION"
mkdir -p "$CACHE/$VERSION"
tar xjf "$CACHE/$ARCHIVE" -C "$CACHE/$VERSION"
src=$(find "$CACHE/$VERSION" -maxdepth 1 -type d -name 'cef_binary_*' | head -1)
[ -n "$src" ] || die "unexpected archive layout"
mv "$src" "$TARGET"
[ -d "$TARGET/Release" ] && { mv "$TARGET"/Release/* "$TARGET"/; rmdir "$TARGET/Release"; }
[ -d "$TARGET/Resources" ] && { mv "$TARGET"/Resources/* "$TARGET"/; rmdir "$TARGET/Resources"; }
printf '{"type":"minimal","name":"cef_binary_%s","sha1":"%s"}\n' "$VERSION" "$SHA1" > "$TARGET/archive.json"

layout_ok || die "extracted SDK looks incomplete: $TARGET"
report
