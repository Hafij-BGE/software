#!/usr/bin/env bash
# Thumbnails for the spreadsheet templates: opens each templates/sheet/*.xlsx in Craft Suite on a
# private X display (Xvfb), takes a picture of the window and keeps the cells (formulas worked
# out, as the app shows them). Run it after changing the spreadsheets, then build_index.py.
#
# Usage: scripts/templates/shoot_sheets.sh [craft-suite binary]   (default: craft-suite on PATH)
# Needs: Xvfb, xwd, and Python with Pillow; ffmpeg converts the picture.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SUITE="${1:-$(command -v craft-suite)}"
WORK="$(mktemp -d)"
trap 'kill "${XVFB:-0}" 2>/dev/null || true; rm -rf "$WORK"' EXIT

export DISPLAY=:87
Xvfb "$DISPLAY" -screen 0 1400x800x24 -nolisten tcp >/dev/null 2>&1 &
XVFB=$!
sleep 1
# A clean profile: the window at its usual size, nothing reopened, no update checks.
# …and its own runtime folder, so the files don't go to a Craft Suite window already open.
mkdir -p "$WORK/home/.config/craft-suite" "$WORK/run"
chmod 700 "$WORK/run"
for f in "$ROOT"/templates/sheet/*.xlsx; do
  XDG_RUNTIME_DIR="$WORK/run" HOME="$WORK/home" XDG_CONFIG_HOME="$WORK/home/.config" XDG_DATA_HOME="$WORK/home/.local/share" \
    "$SUITE" "$f" >/dev/null 2>&1 &
  app=$!
  sleep 15
  xwd -root -silent >"$WORK/shot.xwd"
  kill "$app" 2>/dev/null || true
  wait "$app" 2>/dev/null || true
  ffmpeg -loglevel error -y -i "$WORK/shot.xwd" "$WORK/shot.png"
  # The cells, without the row and column headings.
  python3 - "$WORK/shot.png" "${f%.xlsx}.png" <<'EOF'
import sys
from PIL import Image
im = Image.open(sys.argv[1]).convert("RGB").crop((31, 253, 711, 703))
im = im.resize((360, round(im.height * 360 / im.width)), Image.LANCZOS)
im.save(sys.argv[2], optimize=True)
EOF
  echo "$(basename "${f%.xlsx}.png")"
done
