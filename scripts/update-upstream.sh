#!/usr/bin/env bash
# Bring in new commits from the upstream apps (upstream.conf), merged with your own changes.
#
# Usage: scripts/update-upstream.sh [--check] [app...]      (default: every app in upstream.conf)
#
#   --check   only say which apps have new upstream commits; change nothing
#
# Each app lives in upstream/<app>/ as a git subtree. This runs `git subtree pull --squash`, which
# merges the upstream changes into that folder like any merge: your own commits there are kept,
# and if you and upstream changed the same lines, git stops with a conflict for you to resolve
# (edit the files, `git add`, `git commit`), as in any merge.
#
# Afterwards it re-seeds suite/Cargo.lock from the updated upstream lockfiles
# (scripts/merge-locks.py) and commits that too.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

CHECK=0
ONLY=()
for a in "$@"; do
  case "$a" in
    --check) CHECK=1 ;;
    -h | --help) sed -n '2,15p' "$0"; exit 0 ;;
    -*) echo "unknown option: $a" >&2; exit 2 ;;
    *) ONLY+=("$a") ;;
  esac
done

if [ "$CHECK" = 0 ] && [ -n "$(git status --porcelain --untracked-files=no)" ]; then
  echo "error: you have uncommitted changes; commit or stash them first" >&2
  exit 1
fi

upstream_commit() {
  git log -1 --grep="^git-subtree-dir: upstream/$1\$" --format=%B | sed -n 's/^git-subtree-split: *//p' | head -n1
}

wanted() {
  [ ${#ONLY[@]} -eq 0 ] && return 0
  local o
  for o in "${ONLY[@]}"; do [ "$o" = "$1" ] && return 0; done
  return 1
}

updated=()
while read -r name url branch; do
  case "$name" in '' | \#*) continue ;; esac
  wanted "$name" || continue
  have="$(upstream_commit "$name")"
  head="$(git ls-remote "$url" "refs/heads/$branch" | cut -f1)"
  if [ -z "$head" ]; then
    echo "$name: could not read $branch from $url" >&2
    exit 1
  fi
  if [ "$have" = "$head" ]; then
    echo "$name: up to date (${head:0:12})"
    continue
  fi
  if [ "$CHECK" = 1 ]; then
    echo "$name: new upstream commits (${have:0:12} -> ${head:0:12})"
    updated+=("$name")
    continue
  fi
  echo "==> $name: ${have:0:12} -> ${head:0:12}"
  if ! GIT_LFS_SKIP_SMUDGE=1 git subtree pull --prefix="upstream/$name" --squash "$url" "$branch" \
    -m "Update $name from upstream ($branch, ${head:0:12})"; then
    cat >&2 <<EOF

$name: the upstream changes conflict with changes made in upstream/$name.
Resolve them like any merge conflict:
  git status                  # the conflicted files
  (edit them, keeping both sides as needed)
  git add <files> && git commit
then run this script again for the other apps.
EOF
    exit 1
  fi
  updated+=("$name")
done <upstream.conf

if [ "$CHECK" = 1 ]; then
  [ ${#updated[@]} -gt 0 ] && exit 10
  exit 0
fi

if [ ${#updated[@]} -eq 0 ]; then
  echo "Nothing new upstream."
  exit 0
fi

echo "==> re-seeding suite/Cargo.lock"
python3 -I scripts/merge-locks.py
(cd suite && cargo metadata --format-version 1 >/dev/null)
if ! git diff --quiet -- suite/Cargo.lock; then
  git add suite/Cargo.lock
  git commit -q -m "Refresh suite/Cargo.lock after updating ${updated[*]}"
fi
echo "Updated: ${updated[*]}. Build with packaging/build-deb.sh"
