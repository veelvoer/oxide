#!/usr/bin/env bash
# Builds OxideRemote-<version>.apk without Gradle: cargo-ndk for the Rust code, aapt2 for the manifest and
# resources, javac + d8 for the small Java part (system bars, colors, full screen, updates), zipalign + apksigner.
# Signing: OXIDE_KS, OXIDE_KS_PASS, OXIDE_KEY_ALIAS, OXIDE_KEY_PASS (release), otherwise the Android debug key.
set -euo pipefail
cd "$(dirname "$0")/.."
SDK="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Android/sdk}}"
export ANDROID_NDK_HOME="${ANDROID_NDK_HOME:-$(ls -d "$SDK"/ndk/* | sort -V | tail -1)}"
BT="$(ls -d "$SDK"/build-tools/* | sort -V | tail -1)"
JAR="$(ls -d "$SDK"/platforms/android-* | sort -V | head -1)/android.jar"
VERSION="$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)"
CODE="$(echo "$VERSION" | awk -F. '{print $1*10000+$2*100+$3}')"
OUT="OxideRemote-$VERSION.apk"
B=build
rm -rf $B && mkdir -p $B/apk/lib/arm64-v8a
cargo ndk -t arm64-v8a -o $B/jniLibs build --lib --profile "${PROFILE:-release}"
"$BT/aapt2" compile --dir res -o $B/res.zip
"$BT/aapt2" link -o $B/base.apk -I "$JAR" --manifest AndroidManifest.xml --min-sdk-version 26 --target-sdk-version 35 \
  --version-code "$CODE" --version-name "$VERSION" $B/res.zip
# the Java part: a JDK (javac) from JAVA_HOME, ~/.local/jdk or the PATH
JAVAC="$(ls "${JAVA_HOME:-/nonexistent}"/bin/javac ~/.local/jdk/*/bin/javac 2>/dev/null | head -1 || true)"
JAVAC="${JAVAC:-javac}"
mkdir -p $B/classes $B/dex
"$JAVAC" --release 11 -nowarn -classpath "$JAR" -d $B/classes $(find java -name '*.java')
PATH="$(dirname "$JAVAC"):$PATH" "$BT/d8" --min-api 26 --lib "$JAR" --output $B/dex $(find $B/classes -name '*.class')
cp $B/base.apk $B/unaligned.apk
(cd $B/dex && zip -q -X ../unaligned.apk classes.dex)
cp $B/jniLibs/arm64-v8a/liboxide_remote.so $B/apk/lib/arm64-v8a/
(cd $B/apk && zip -q -X -0 ../unaligned.apk lib/arm64-v8a/liboxide_remote.so)
"$BT/zipalign" -f -P 16 4 $B/unaligned.apk $B/aligned.apk
if [ -n "${OXIDE_KS:-}" ]; then
  "$BT/apksigner" sign --ks "$OXIDE_KS" --ks-pass "pass:$OXIDE_KS_PASS" --ks-key-alias "$OXIDE_KEY_ALIAS" --key-pass "pass:$OXIDE_KEY_PASS" --out "$OUT" $B/aligned.apk
else
  KS="$HOME/.android/debug.keystore"
  [ -f "$KS" ] || { mkdir -p "$HOME/.android"; keytool -genkeypair -keystore "$KS" -storepass android -keypass android -alias androiddebugkey -dname "CN=Android Debug" -keyalg RSA -keysize 2048 -validity 10000 >/dev/null 2>&1; }
  "$BT/apksigner" sign --ks "$KS" --ks-pass pass:android --ks-key-alias androiddebugkey --key-pass pass:android --out "$OUT" $B/aligned.apk
fi
"$BT/apksigner" verify "$OUT"
echo "built $OUT ($(du -h "$OUT" | cut -f1))"
