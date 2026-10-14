#!/usr/bin/env bash
# Adds files to this commit's GitHub release (tag v<version>, "+" spelled "-"), from GitHub Actions.
#
# Usage: packaging/publish-release.sh --create FILE...   create the release, then add FILE...
#        packaging/publish-release.sh FILE...            wait for the release (made by the .deb
#                                                        job), then add FILE...
#
# The .deb job creates the release, so a release only appears with the .deb and latest.txt that
# installed copies look for; the Windows and Android jobs add theirs when it's there (waiting up
# to RELEASE_WAIT_MINUTES, default 120).
#
# Environment (GitHub Actions sets these): GH_TOKEN, GITHUB_REPOSITORY, GITHUB_SHA, and
# GITHUB_API_URL (default https://api.github.com).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
. "$ROOT/packaging/lib.sh"

CREATE=0
if [ "${1:-}" = "--create" ]; then CREATE=1; shift; fi
[ $# -gt 0 ] || { echo "usage: $0 [--create] FILE..." >&2; exit 2; }
: "${GH_TOKEN:?GH_TOKEN is needed}" "${GITHUB_REPOSITORY:?}" "${GITHUB_SHA:?}"

API="${GITHUB_API_URL:-https://api.github.com}/repos/$GITHUB_REPOSITORY"
UPLOADS="${GITHUB_UPLOADS_URL:-https://uploads.github.com}/repos/$GITHUB_REPOSITORY"
TAG="v${VERSION//+/-}"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
umask 077
printf 'Authorization: Bearer %s\n' "$GH_TOKEN" >"$tmp/auth"

# release_id: the id of the release with $TAG, or nothing.
release_id() {
  local status
  status="$(curl -sS -H @"$tmp/auth" -H "Accept: application/vnd.github+json" \
    -o "$tmp/release.json" -w '%{http_code}' "$API/releases/tags/$TAG" || true)"
  # The release's own "id" comes before any nested one (author, assets). No python3 here: the
  # Windows job runs this in Git Bash.
  if [ "$status" = 200 ]; then
    grep -o '"id": *[0-9]*' "$tmp/release.json" | head -n1 | grep -o '[0-9]*$'
  fi
}

if [ "$CREATE" = 1 ]; then
  python3 - "$TAG" "$GITHUB_SHA" "$VERSION" >"$tmp/body.json" <<'EOF'
import json, sys
tag, sha, version = sys.argv[1:4]
v = version.replace("+", "-")
body = f"""Built from {sha[:12]}.

**Linux** (Ubuntu 22.04+, Debian 12+): `sudo apt install ./craft-suite_{v}_amd64.deb`. Installed copies update with **Check for updates** on the Home page, or `craft-suite-update`.

**Windows** (64-bit): run `craft-suite_{v}_windows-x64-setup.exe`, or unzip `craft-suite_{v}_windows-x64.zip` and run `craft-suite.exe`.

**Android** (8.0+): install `craft-suite_{v}.apk` (allow installing apps from your browser or file manager when Android asks).

The Windows and Android files are added a few minutes after the Linux package."""
print(json.dumps({"tag_name": tag, "target_commitish": sha, "name": "Craft Suite " + version, "body": body}))
EOF
  id="$(curl -fsS -X POST -H @"$tmp/auth" -H "Accept: application/vnd.github+json" "$API/releases" \
    --data-binary @"$tmp/body.json" | python3 -c 'import json,sys; print(json.load(sys.stdin)["id"])')"
  echo "created release $TAG ($id)"
else
  wait="${RELEASE_WAIT_MINUTES:-120}"
  id=""
  for _ in $(seq 1 "$wait"); do
    id="$(release_id)"
    [ -n "$id" ] && break
    echo "waiting for release $TAG (made when the .deb is built)…"
    sleep 60
  done
  [ -n "$id" ] || { echo "no release $TAG after $wait minutes" >&2; exit 1; }
fi

for f in "$@"; do
  name="$(basename "$f")"
  curl -fsS -X POST -H @"$tmp/auth" -H "Content-Type: application/octet-stream" \
    --data-binary @"$f" "$UPLOADS/releases/$id/assets?name=$name" >/dev/null
  echo "added $name to $TAG"
done
