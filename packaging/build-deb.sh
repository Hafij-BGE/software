#!/usr/bin/env bash
# Build Craft Suite and pack it into one Debian package:
#
#   dist/craft-suite_<version>_<arch>.deb
#
# It holds craft-suite (the tabbed window: WordCraft, DeckCraft, GridCraft and Markd in one), the
# three command-line tools (wordcraft-cli, deckcraft-cli, gridcraft-cli), Markd's web files,
# craft-suite-update, desktop entries, file types and icons.
#
# Usage: packaging/build-deb.sh [--skip-build] [--with-fonts] [--update-url URL]
#
#   --skip-build    package what's already in $CARGO_TARGET_DIR/release
#   --with-fonts    embed the craft-fonts each upstream release embeds (Japanese and Chinese UI
#                   text among them): clones github.com/storytold/craft-fonts at the commit each
#                   app pins in its release workflow
#   --update-url    where craft-suite-update looks for releases (default: this repository's
#                   GitHub releases, from GITHUB_REPOSITORY or the origin remote)
#
# Environment: CRAFT_SUITE_VERSION (default: VERSION file + "+r<commit time, UTC>"),
# CRAFT_SUITE_MAINTAINER, CARGO_TARGET_DIR (default: target/), DIST (default: dist/).
#
# Needs: cargo, dpkg-deb, the build libraries listed in README.md, and Node.js with pnpm for
# Markd (packaging/build-markd-web.sh).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HERE="$ROOT/packaging"
APPS=(wordcraft deckcraft gridcraft)

SKIP_BUILD=0
WITH_FONTS=0
UPDATE_URL=""
while [ $# -gt 0 ]; do
  case "$1" in
    --skip-build) SKIP_BUILD=1; shift ;;
    --with-fonts) WITH_FONTS=1; shift ;;
    --update-url) UPDATE_URL="$2"; shift 2 ;;
    -h | --help) sed -n '2,21p' "$0"; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"
DIST="${DIST:-$ROOT/dist}"
mkdir -p "$DIST"

# The version (VERSION + the commit's time) and the upstream helpers.
. "$HERE/lib.sh"
ARCH="$(dpkg --print-architecture)"
MAINTAINER="${CRAFT_SUITE_MAINTAINER:-Md. Hafijur Rahman <craft-suite@localhost>}"

if [ -z "$UPDATE_URL" ]; then
  REPO="${GITHUB_REPOSITORY:-}"
  if [ -z "$REPO" ]; then
    REPO="$(git -C "$ROOT" remote get-url origin 2>/dev/null | sed -nE 's#.*github\.com[:/]([^/]+/[^/.]+)(\.git)?/?$#\1#p')"
  fi
  if [ -n "$REPO" ]; then UPDATE_URL="https://github.com/$REPO/releases/latest/download"; fi
fi

echo "==> Craft Suite $VERSION ($ARCH)"
craft_libs_fallback

if [ "$SKIP_BUILD" = 0 ]; then
  for app in "${APPS[@]}"; do
    sha="$(upstream_commit "$app")"
    fonts=""
    if [ "$WITH_FONTS" = 1 ] && [ -n "$(fonts_ref "$app")" ]; then fonts="$(fonts_dir "$(fonts_ref "$app")")"; fi
    upper="$(echo "$app" | tr '[:lower:]' '[:upper:]')"
    echo "==> $app-cli (upstream ${sha:0:8})"
    (cd "$ROOT/upstream/$app" &&
      env "${upper}_BUILD_SHA=$sha" ${fonts:+CRAFT_FONTS_DIR="$fonts"} \
        cargo build --release --locked -p "$app-cli")
  done
  # The tabbed window links all three apps' libraries; WordCraft's and DeckCraft's font crates
  # both read CRAFT_FONTS_DIR, so they get WordCraft's pin.
  fonts=""
  if [ "$WITH_FONTS" = 1 ] && [ -n "$(fonts_ref wordcraft)" ]; then fonts="$(fonts_dir "$(fonts_ref wordcraft)")"; fi
  echo "==> craft-suite"
  (cd "$ROOT/suite" &&
    env WORDCRAFT_BUILD_SHA="$(upstream_commit wordcraft)" ${fonts:+CRAFT_FONTS_DIR="$fonts"} \
      cargo build --release)
  "$HERE/build-markd-web.sh" "$ROOT/build/markd/www"
fi
if [ ! -f "$ROOT/build/markd/www/index.html" ]; then
  echo "error: no build/markd/www (run packaging/build-markd-web.sh)" >&2
  exit 1
fi
BIN="$CARGO_TARGET_DIR/release"

# ---- stage the file tree ------------------------------------------------------------------------
WORK="$ROOT/build/deb"
STAGE="$WORK/craft-suite"
rm -rf "$WORK"
mkdir -p "$STAGE/DEBIAN"

install -Dm755 "$BIN/craft-suite" "$STAGE/usr/bin/craft-suite"
for app in "${APPS[@]}"; do
  install -Dm755 "$BIN/$app-cli" "$STAGE/usr/bin/$app-cli"
done
strip "$STAGE"/usr/bin/* 2>/dev/null || true
install -Dm755 "$HERE/craft-suite-update" "$STAGE/usr/bin/craft-suite-update"

for f in "$HERE"/craft-suite*.desktop; do
  install -Dm644 "$f" "$STAGE/usr/share/applications/$(basename "$f")"
done
mkdir -p "$STAGE/usr/share/icons"
cp -R "$ROOT/assets/hicolor" "$STAGE/usr/share/icons/"
for d in "$ROOT"/upstream/markd/packages/desktop/build/icons/*x*/; do
  size="$(basename "$d")"
  install -Dm644 "$d/marktext.png" "$STAGE/usr/share/icons/hicolor/$size/apps/craft-suite-markd.png"
done
# The apps' icons are the suite's own (scripts/make-app-icons.py), also under the upstream names
# the file types use (ai.storyteller.<app>), so files show them too.
for pair in wordcraft:word deckcraft:slide gridcraft:sheet; do
  app="${pair%%:*}" icon="${pair##*:}"
  for f in "$ROOT"/assets/hicolor/*/apps/craft-suite-$icon.*; do
    dir="$(dirname "$f")"
    install -Dm644 "$f" "$STAGE/usr/share/icons/hicolor/${dir#"$ROOT/assets/hicolor/"}/ai.storyteller.$app.${f##*.}"
  done
  install -Dm644 "$ROOT/upstream/$app/packaging/linux/ai.storyteller.$app.mime.xml" \
    "$STAGE/usr/share/mime/packages/ai.storyteller.$app.xml"
done

# What this build is, for the About section and craft-suite-update.
mkdir -p "$STAGE/usr/share/craft-suite"
cp -R "$ROOT/build/markd/www" "$STAGE/usr/share/craft-suite/markd"
echo "$VERSION" >"$STAGE/usr/share/craft-suite/VERSION"
upstream_summary >"$STAGE/usr/share/craft-suite/upstream.txt"

mkdir -p "$STAGE/etc/craft-suite"
cat >"$STAGE/etc/craft-suite/update.conf" <<EOF
# Where craft-suite-update finds new releases: a folder with latest.txt and the .deb files it
# lists (see packaging/craft-suite-update). ~/.config/craft-suite/update.conf overrides this.
# A private GitHub repository also needs a read-only token, in your own
# ~/.config/craft-suite/update.conf (never here, where every user can read it):
#   TOKEN="github_pat_..."
URL="$UPDATE_URL"
EOF

# Licences: the suite's and each app's (MIT OR Apache-2.0), plus the fonts' when embedded.
DOC="$STAGE/usr/share/doc/craft-suite"
mkdir -p "$DOC"
cp "$ROOT/README.md" "$DOC/"
for app in "${APPS[@]}"; do
  for f in LICENSE-MIT LICENSE-APACHE NOTICE ATTRIBUTION.md; do
    if [ -f "$ROOT/upstream/$app/$f" ]; then cp "$ROOT/upstream/$app/$f" "$DOC/$app-$f"; fi
  done
done
cp "$ROOT/upstream/markd/LICENSE" "$DOC/markd-LICENSE"
if [ "$WITH_FONTS" = 1 ]; then
  for d in "$ROOT"/build/craft-fonts-*/fonts/*/; do
    if [ -f "$d/OFL.txt" ]; then cp "$d/OFL.txt" "$DOC/OFL-$(basename "$d").txt"; fi
  done
fi
cat >"$DOC/copyright" <<'EOF'
Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/
Upstream-Name: craft-suite
Comment: WordCraft, DeckCraft and GridCraft (https://github.com/storytold) by the ArtCraft team,
 and Markd (https://github.com/Hafij-BGE/markd, based on MarkText), combined in one tabbed
 window. Each app's licence files are in this folder as <app>-LICENSE*.

Files: *
License: MIT or Apache-2.0

Files: usr/share/craft-suite/markd/*
License: MIT
EOF

# ---- control files ------------------------------------------------------------------------------
# The binaries need at least the glibc of the build host's newest symbol they use.
GLIBC="$(objdump -T "$STAGE"/usr/bin/* 2>/dev/null | grep -o 'GLIBC_[0-9][0-9.]*' | sed 's/GLIBC_//' | sort -uV | tail -n1 || true)"
GLIBC="${GLIBC:-$(dpkg-query -W -f='${Version}' libc6 | sed 's/-.*//')}"
SIZE="$(du -sk --exclude=DEBIAN "$STAGE" | cut -f1)"
cat >"$STAGE/DEBIAN/control" <<EOF
Package: craft-suite
Version: $VERSION
Architecture: $ARCH
Maintainer: $MAINTAINER
Installed-Size: $SIZE
Depends: libc6 (>= $GLIBC), libgcc-s1, libxkbcommon0, libxkbcommon-x11-0, libasound2t64 | libasound2, libvulkan1 | libegl1, libwebkit2gtk-4.1-0, libgtk-3-0t64 | libgtk-3-0
Recommends: libwayland-client0, libx11-6, libx11-xcb1, libxcursor1, libxi6, libxrandr2, mesa-vulkan-drivers, xdg-desktop-portal, curl
Suggests: zenity
Conflicts: wordcraft, deckcraft, gridcraft
Replaces: wordcraft, deckcraft, gridcraft
Section: editors
Priority: optional
Homepage: https://github.com/storytold
Description: Word, Slide, Spreadsheet and Markd in one tabbed window
 Craft Suite puts four open-source apps in one window, one document per tab:
 Word (documents: .docx, .odt, .rtf), Slide (PowerPoint presentations:
 .pptx), Spreadsheet (Excel workbooks: .xlsx, .ods, .csv), built from ArtCraft's
 WordCraft, DeckCraft and GridCraft, and Markd (Markdown: .md, with HTML, PDF
 and Word export). Tabs of different apps
 can be mixed, grouped or kept in a row per app, along any edge of the window,
 with app colours and per-tab colours.
 .
 Includes wordcraft-cli, deckcraft-cli and gridcraft-cli, headless converters
 and command runners, and craft-suite-update, which installs newer releases.
 .
 Created by Md. Hafijur Rahman. Source: WordCraft, DeckCraft and GridCraft by
 ArtCraft (https://github.com/storytold); Markd (https://github.com/Hafij-BGE/markd),
 based on MarkText.
EOF
echo "/etc/craft-suite/update.conf" >"$STAGE/DEBIAN/conffiles"
install -m755 "$HERE/postinst" "$STAGE/DEBIAN/postinst"
install -m755 "$HERE/postinst" "$STAGE/DEBIAN/postrm"

# The file name spells "+" as "-": "+" in a download or upload URL can turn into a space, and
# GitHub renames release files with such characters. The package's own version keeps the "+".
OUT="$DIST/craft-suite_${VERSION//+/-}_${ARCH}.deb"
dpkg-deb --root-owner-group -Zxz --build "$STAGE" "$OUT"
# For craft-suite-update: "<arch> <version> <file> <sha256>" (CI publishes it with the release).
echo "$ARCH $VERSION $(basename "$OUT") $(sha256sum "$OUT" | cut -d' ' -f1)" >"$DIST/latest-$ARCH.txt"
echo "==> wrote $OUT"
ls -lh "$OUT"
