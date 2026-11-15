#!/bin/sh
# Chromium's setuid sandbox helper ships without the setuid bit (a build tree does not keep
# one), and CEF refuses to start in that state:
#   The SUID sandbox helper binary was found, but is not configured correctly.
# The helper is installed next to the binary.
set -e
for helper in /usr/bin/chrome-sandbox /usr/lib/openclion/chrome-sandbox; do
  if [ -f "$helper" ]; then
    chown root:root "$helper"
    chmod 4755 "$helper"
  fi
done
exit 0
