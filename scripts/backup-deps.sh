#!/usr/bin/env bash
# Keeps backup/ up to date: copies of what the builds download from ArtCraft's GitHub
# (github.com/storytold), so Craft Suite still builds if those repositories ever disappear.
#
#   backup/craft-libs.bundle    the craft-libs repository (branches, tags, full history, and each
#                               commit the Cargo.lock files pin), as one git bundle file
#   backup/craft-fonts/<ref>/   the fonts of each craft-fonts commit an app's release embeds
#
# The builds use github.com as before; only when it can't be reached do packaging/lib.sh's helpers
# take these copies instead (same commits, so Cargo.lock still matches). The daily upstream sync
# runs this after merging an update. When storytold can't be reached, the backup is left as it is.
#
# Usage: scripts/backup-deps.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
. "$ROOT/packaging/lib.sh"
BACKUP="$ROOT/backup"
LIBS_URL=https://github.com/storytold/craft-libs
mkdir -p "$BACKUP/craft-fonts"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

# ---- craft-libs ---------------------------------------------------------------------------------
revs="$(grep -ho "git+$LIBS_URL?rev=[0-9a-f]*" "$ROOT"/suite/Cargo.lock "$ROOT"/upstream/*/Cargo.lock 2>/dev/null |
  sed 's/.*rev=//' | sort -u)"
if git clone -q --bare "$LIBS_URL" "$tmp/craft-libs.git" 2>/dev/null; then
  for rev in $revs; do
    # A pinned commit that no branch contains any more still goes in the bundle.
    git -C "$tmp/craft-libs.git" fetch -q origin "$rev" 2>/dev/null || true
    git -C "$tmp/craft-libs.git" cat-file -e "$rev^{commit}" || { echo "craft-libs has no commit $rev" >&2; exit 1; }
    git -C "$tmp/craft-libs.git" update-ref "refs/pins/$rev" "$rev"
  done
  git -C "$tmp/craft-libs.git" bundle create -q "$tmp/craft-libs.bundle" --all
  if [ -f "$BACKUP/craft-libs.bundle" ] &&
    [ "$(git bundle list-heads "$tmp/craft-libs.bundle" | sort)" = "$(git bundle list-heads "$BACKUP/craft-libs.bundle" | sort)" ]; then
    echo "backup/craft-libs.bundle: up to date"
  else
    mv "$tmp/craft-libs.bundle" "$BACKUP/craft-libs.bundle"
    echo "backup/craft-libs.bundle: refreshed ($(du -h "$BACKUP/craft-libs.bundle" | cut -f1))"
  fi
else
  echo "warning: $LIBS_URL can't be reached; backup/craft-libs.bundle kept as it is" >&2
fi

# ---- craft-fonts --------------------------------------------------------------------------------
pinned=""
for app in wordcraft deckcraft gridcraft; do
  ref="$(fonts_ref "$app")"
  [ -n "$ref" ] || continue
  pinned="$pinned $ref"
  dest="$BACKUP/craft-fonts/$ref"
  if [ -f "$dest/fonts/manifest.txt" ]; then
    continue
  fi
  if dir="$(fonts_dir "$ref")" && [ "$dir" != "$dest" ]; then
    rm -rf "$dest"
    mkdir -p "$dest"
    cp -R "$dir/fonts" "$dest/"
    echo "backup/craft-fonts/$ref: added ($app)"
  else
    echo "warning: craft-fonts $ref can't be fetched; not in the backup" >&2
  fi
done
# Font commits no app pins any more (the repository's history keeps them).
for d in "$BACKUP"/craft-fonts/*/; do
  [ -d "$d" ] || continue
  ref="$(basename "$d")"
  case " $pinned " in *" $ref "*) ;; *) rm -rf "$d"; echo "backup/craft-fonts/$ref: no longer pinned, removed" ;; esac
done
