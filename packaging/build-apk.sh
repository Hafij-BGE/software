#!/usr/bin/env bash
# Builds Craft Suite for Android: dist/craft-suite_<version>.apk, a small app (android/) that shows
# WordCraft, DeckCraft and GridCraft's web versions (upstream/*/apps/*-web, WebAssembly) in a
# WebView, from the APK itself (no website, works offline).
#
# Usage: packaging/build-apk.sh [--skip-web]
#   --skip-web   use the web builds already in build/web/{word,deck,grid}
#
# Needs: Rust with the wasm32-unknown-unknown target, trunk (https://trunkrs.dev), a JDK (javac,
# keytool), zip, and the Android SDK ($ANDROID_HOME or $ANDROID_SDK_ROOT) with a platform
# (android-26 or newer) and build-tools.
#
# Signing: ANDROID_KEYSTORE (a .jks file), ANDROID_KEYSTORE_PASSWORD and ANDROID_KEY_ALIAS
# (default craft-suite). Without them a throwaway key signs the APK: it installs, but Android only
# installs an update over it when both are signed with the same key, so keep one key for good.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SKIP_WEB=0
for a in "$@"; do
  case "$a" in
    --skip-web) SKIP_WEB=1 ;;
    -h | --help) sed -n '2,17p' "$0"; exit 0 ;;
    *) echo "unknown argument: $a" >&2; exit 2 ;;
  esac
done

# The same version as the .deb: VERSION plus the commit's UTC time (lib.sh).
. "$ROOT/packaging/lib.sh"
# Android's versionCode must grow with each release: the commit time in minutes.
VERSION_CODE="$(( $(git -C "$ROOT" log -1 --format=%ct 2>/dev/null || date +%s) / 60 ))"

SDK="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-}}"
[ -n "$SDK" ] && [ -d "$SDK" ] || { echo "set ANDROID_HOME to the Android SDK" >&2; exit 1; }
BT="$(ls -d "$SDK"/build-tools/* | sort -V | tail -n1)"
PLATFORM="$(ls -d "$SDK"/platforms/android-* | sort -V | tail -n1)"
ANDROID_JAR="$PLATFORM/android.jar"
echo "build-tools: $BT"
echo "platform:    $PLATFORM"

WEB="$ROOT/build/web"
if [ "$SKIP_WEB" = 0 ]; then
  for pair in word:wordcraft deck:deckcraft grid:gridcraft; do
    short="${pair%%:*}" app="${pair##*:}"
    echo "== $app web"
    (cd "$ROOT/upstream/$app/apps/$app-web" && trunk build --release --dist "$WEB/$short")
  done
fi

STAGE="$ROOT/build/apk"
DIST="${DIST:-$ROOT/dist}"
rm -rf "$STAGE"
mkdir -p "$STAGE/assets/web/icons" "$STAGE/classes" "$STAGE/dex" "$DIST"

# The pages: the launcher, the shim that hands downloads to the app, and the three web apps, each
# loading the shim before its own scripts.
for short in word deck grid; do
  [ -f "$WEB/$short/index.html" ] || { echo "missing $WEB/$short (run without --skip-web)" >&2; exit 1; }
  cp -r "$WEB/$short" "$STAGE/assets/web/$short"
  python3 - "$STAGE/assets/web/$short/index.html" <<'EOF'
import re, sys
p = sys.argv[1]
s = open(p, encoding="utf-8").read()
s, n = re.subn(r"(<head[^>]*>)", r'\1<script src="../android-shim.js"></script>', s, count=1)
if n != 1:
    sys.exit(f"no <head> in {p}")
open(p, "w", encoding="utf-8").write(s)
EOF
done
cp "$ROOT/android/web/android-shim.js" "$STAGE/assets/web/"
sed "s/@VERSION@/$VERSION/" "$ROOT/android/web/index.html" > "$STAGE/assets/web/index.html"
cp "$ROOT/assets/hicolor/128x128/apps/craft-suite.png" "$STAGE/assets/web/icons/craft-suite.png"
for app in wordcraft deckcraft gridcraft; do
  cp "$ROOT/upstream/$app/assets/app-icon/hicolor/128x128/apps/ai.storyteller.$app.png" "$STAGE/assets/web/icons/$app.png"
done

echo "== compile"
javac --release 11 -nowarn -cp "$ANDROID_JAR" -d "$STAGE/classes" $(find "$ROOT/android/src" -name '*.java')
"$BT/d8" --release --min-api 26 --lib "$ANDROID_JAR" --output "$STAGE/dex" $(find "$STAGE/classes" -name '*.class')

echo "== package"
"$BT/aapt2" compile --dir "$ROOT/android/res" -o "$STAGE/res.zip"
"$BT/aapt2" link -o "$STAGE/unsigned.apk" -I "$ANDROID_JAR" \
  --manifest "$ROOT/android/AndroidManifest.xml" -R "$STAGE/res.zip" -A "$STAGE/assets" \
  --min-sdk-version 26 --target-sdk-version 34 \
  --version-code "$VERSION_CODE" --version-name "$VERSION"
(cd "$STAGE/dex" && zip -q "$STAGE/unsigned.apk" classes.dex)
"$BT/zipalign" -p -f 4 "$STAGE/unsigned.apk" "$STAGE/aligned.apk"

echo "== sign"
KEYSTORE="${ANDROID_KEYSTORE:-}"
ALIAS="${ANDROID_KEY_ALIAS:-craft-suite}"
if [ -z "$KEYSTORE" ] || [ ! -s "$KEYSTORE" ]; then
  echo "warning: no ANDROID_KEYSTORE; signing with a throwaway key (updates will need an uninstall first)" >&2
  KEYSTORE="$STAGE/throwaway.jks"
  ALIAS=craft-suite
  export ANDROID_KEYSTORE_PASSWORD="throwaway-$RANDOM$RANDOM"
  keytool -genkeypair -keystore "$KEYSTORE" -storepass "$ANDROID_KEYSTORE_PASSWORD" -keypass "$ANDROID_KEYSTORE_PASSWORD" \
    -alias "$ALIAS" -keyalg RSA -keysize 2048 -validity 10000 -dname "CN=Craft Suite" >/dev/null 2>&1
fi
OUT="$DIST/craft-suite_${VERSION//+/-}.apk"
"$BT/apksigner" sign --ks "$KEYSTORE" --ks-pass env:ANDROID_KEYSTORE_PASSWORD --ks-key-alias "$ALIAS" \
  --out "$OUT" "$STAGE/aligned.apk"
"$BT/apksigner" verify "$OUT"
echo "$OUT ($(du -h "$OUT" | cut -f1), versionCode $VERSION_CODE)"
