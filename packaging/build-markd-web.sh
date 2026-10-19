#!/usr/bin/env bash
# Builds Markd's web version (upstream/markd/packages/android: the muya editor with Markd's
# ribbon, themes, page layout and exports), which the suite's Markd tabs show.
#
# Usage: packaging/build-markd-web.sh [OUT_DIR] [BASE]
#
#   OUT_DIR   where the files go (default: build/markd/www, where `cargo run` in suite/ finds them)
#   BASE      the path the page is served from (default: /)
#
# Needs: Node.js 20.19 or newer and pnpm 10 (`corepack enable`; upstream/markd/package.json pins
# the version). Installs Markd's JavaScript dependencies into upstream/markd/node_modules
# (ignored by git) without running their install scripts (no Electron download).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="${1:-$ROOT/build/markd/www}"
BASE="${2:-/}"
case "$OUT" in /*) ;; *) OUT="$PWD/$OUT" ;; esac

if command -v pnpm >/dev/null 2>&1; then PNPM=(pnpm); else PNPM=(corepack pnpm); fi

cd "$ROOT/upstream/markd"
echo "==> markd web ($(git -C "$ROOT" log -1 --format=%h -- upstream/markd 2>/dev/null || echo unknown)) -> $OUT"
"${PNPM[@]}" install --frozen-lockfile --ignore-scripts --filter "@markd/android..."
"${PNPM[@]}" --filter @muyajs/core build:types
cd packages/android
"${PNPM[@]}" exec tsc --noEmit
"${PNPM[@]}" exec vite build --base "$BASE" --outDir "$OUT" --emptyOutDir
test -f "$OUT/index.html"
