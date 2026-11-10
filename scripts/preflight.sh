#!/usr/bin/env bash
# The checks that broke CI after past upstream updates, run locally before a push.
#
# Usage: scripts/preflight.sh [--quick] [--install-hook]
#
#   (none)          lock files, fork pins, then `cargo check` of every app and the suite (minutes)
#   --quick         lock files and fork pins only (seconds); the pre-push hook runs this
#   --install-hook  make `git push` run `scripts/preflight.sh --quick` first
#
# What it catches, and the CI failure each check stands for:
#   1. upstream/<app>/Cargo.lock out of date with its Cargo.toml. packaging/build-deb.sh and
#      build-windows.sh build each app with --locked, so CI fails with "cannot update the lock
#      file ... because --locked was passed". This happens when a subtree pull brings upstream's
#      lock file but the suite has added a dependency (craft-font-files) to that workspace.
#      Fix: (cd upstream/<app> && cargo metadata --format-version 1 >/dev/null), commit the lock.
#   2. The apps pin different revisions of a forked crate (winit, egui, krilla) in
#      [patch.crates-io], or suite/Cargo.toml lags WordCraft's. The suite links all apps against
#      one revision, so it fails with errors like "WindowEvent::ImeResetCompleted not covered" or
#      "cannot find value CRAFT_... in crate backend". Fix: bring the pins in line.
#   3. Compile errors in the apps or the suite (an upstream API the suite uses changed).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

QUICK=0
for a in "$@"; do
  case "$a" in
    --quick) QUICK=1 ;;
    --install-hook)
      hooks="$(git rev-parse --git-path hooks)"
      mkdir -p "$hooks"
      printf '#!/bin/sh\nexec "$(git rev-parse --show-toplevel)/scripts/preflight.sh" --quick\n' >"$hooks/pre-push"
      chmod +x "$hooks/pre-push"
      echo "installed $hooks/pre-push"
      exit 0
      ;;
    -h | --help) sed -n '2,21p' "$0"; exit 0 ;;
    *) echo "unknown option: $a" >&2; exit 2 ;;
  esac
done

fail=0
log="$(mktemp)"
trap 'rm -f "$log"' EXIT

echo "==> lock files (--locked builds)"
for toml in upstream/*/Cargo.toml; do
  ws="$(dirname "$toml")"
  if (cd "$ws" && cargo metadata --locked --format-version 1 >/dev/null 2>"$log"); then
    echo "  ok    $ws/Cargo.lock"
  else
    echo "  FAIL  $ws/Cargo.lock is out of date with $ws/Cargo.toml"
    grep -m1 -E '^error' "$log" | sed 's/^/        /' || true
    echo "        fix: (cd $ws && cargo metadata --format-version 1 >/dev/null) and commit the lock"
    fail=1
  fi
done

echo "==> fork pins ([patch.crates-io])"
if ! python3 -I - <<'PY'; then fail=1; fi
import pathlib, re, sys, tomllib
pins = {}
for path in sorted(pathlib.Path(".").glob("upstream/*/Cargo.toml")) + [pathlib.Path("suite/Cargo.toml")]:
    patch = tomllib.loads(path.read_text()).get("patch", {}).get("crates-io", {})
    for name, spec in patch.items():
        if isinstance(spec, dict) and "git" in spec:
            pins.setdefault(name, {})[str(path)] = spec.get("rev") or spec.get("branch") or spec.get("tag") or "?"
bad = 0
for name, where in sorted(pins.items()):
    if len(set(where.values())) > 1:
        bad = 1
        print(f"  FAIL  {name} is pinned to different revisions:")
        for p, rev in where.items():
            print(f"          {rev[:12]}  {p}")
if bad:
    print("        fix: give the apps and suite/Cargo.toml one revision (the newest the apps build with)")
else:
    print(f"  ok    {len(pins)} forked crates, one revision each")
sys.exit(bad)
PY

if [ "$QUICK" = 0 ]; then
  for toml in upstream/*/Cargo.toml; do
    ws="$(dirname "$toml")"
    app="$(basename "$ws")"
    echo "==> cargo check $app-cli"
    if ! (cd "$ws" && cargo check --locked -p "$app-cli"); then fail=1; fi
  done
  echo "==> cargo check suite"
  if ! (cd suite && cargo check); then fail=1; fi
fi

if [ "$fail" = 1 ]; then
  echo "preflight failed: fix the above before pushing (git push --no-verify skips this check)" >&2
  exit 1
fi
echo "preflight ok"
