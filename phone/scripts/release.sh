#!/usr/bin/env bash
# Build the APK and publish it on GitHub, where the app's "Update" button finds it.
# Needs `gh` (logged in). Updates install only over an app signed with the same key: keep the key you used
# (~/.android/debug.keystore by default, or OXIDE_KS & co., see build-apk.sh).
set -euo pipefail
cd "$(dirname "$0")/.."
VERSION="$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)"
bash scripts/build-apk.sh
gh release create "phone-v$VERSION" "OxideRemote-$VERSION.apk" \
  --title "Oxide Remote $VERSION" \
  --notes "The Oxide Remote app for Android. Install it, or press Update in the app (Settings > Updates)."
