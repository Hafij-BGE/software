# Shared by build-deb.sh and build-windows.sh: the version, and what each upstream
# app is built from. Sourced with ROOT set to the repository.

# ---- version: VERSION + the commit's time (UTC, to the minute), so a newer commit sorts higher
# however it reached the branch (a squash merge leaves few commits, so a commit count could go
# down). "r" first: older builds were "+r<count>", and dpkg sorts "+r..." above a bare "+2026...".
BASE="$(tr -d ' \n\r' <"$ROOT/VERSION")"
STAMP="$(TZ=UTC git -C "$ROOT" log -1 --format=%cd --date=format-local:%Y%m%d%H%M 2>/dev/null || date -u +%Y%m%d%H%M)"
VERSION="${CRAFT_SUITE_VERSION:-$BASE+r$STAMP}"

# The upstream commit each app was last merged from (git subtree records it).
upstream_commit() {
  git -C "$ROOT" log -1 --grep="^git-subtree-dir: upstream/$1\$" --format=%B 2>/dev/null |
    sed -n 's/^git-subtree-split: *//p' | head -n1
}
upstream_version() {
  if [ -f "$ROOT/upstream/$1/Cargo.toml" ]; then
    awk '/^\[/ { p = ($0 == "[workspace.package]"); next } p && $1 == "version" { gsub(/[" \r]/, "", $3); print $3; exit }' \
      "$ROOT/upstream/$1/Cargo.toml"
  else
    # Markd: the desktop app's package.json.
    sed -n 's/^  "version": "\([^"]*\)".*/\1/p' "$ROOT/upstream/$1/packages/desktop/package.json" 2>/dev/null | head -n1
  fi
}
# The craft-fonts commit an app's release workflow embeds.
fonts_ref() {
  sed -n 's/^ *craft-fonts-ref: *\([0-9a-f]\{7,40\}\).*/\1/p' "$ROOT/upstream/$1/.github/workflows/release.yml" 2>/dev/null | head -n1
}
# The craft-fonts at `ref`, from github.com/storytold/craft-fonts, or from backup/craft-fonts when
# that can't be reached (scripts/backup-deps.sh keeps it).
fonts_dir() {
  local ref="$1" dir="$ROOT/build/craft-fonts-$1" backup="$ROOT/backup/craft-fonts/$1"
  if [ ! -f "$dir/fonts/manifest.txt" ]; then
    rm -rf "$dir"
    git init -q "$dir"
    if git -C "$dir" fetch -q --depth 1 https://github.com/storytold/craft-fonts "$ref" 2>/dev/null; then
      git -C "$dir" -c advice.detachedHead=false checkout -q FETCH_HEAD
    elif [ -f "$backup/fonts/manifest.txt" ]; then
      echo "github.com/storytold/craft-fonts can't be reached: using backup/craft-fonts/$ref" >&2
      rm -rf "$dir"
      mkdir -p "$dir"
      cp -R "$backup/fonts" "$dir/"
    else
      echo "craft-fonts $ref: github.com/storytold/craft-fonts can't be reached, and it isn't in backup/" >&2
      return 1
    fi
  fi
  echo "$dir"
}

# When github.com/storytold/craft-libs can't be reached, Cargo fetches it from
# backup/craft-libs.bundle instead: the same commits under the same URL, so the Cargo.lock files
# still match. Call before building.
craft_libs_fallback() {
  local url=https://github.com/storytold/craft-libs bundle="$ROOT/backup/craft-libs.bundle"
  if git ls-remote -q "$url" HEAD >/dev/null 2>&1; then
    return 0
  fi
  if [ ! -f "$bundle" ]; then
    echo "warning: $url can't be reached, and backup/craft-libs.bundle is missing" >&2
    return 0
  fi
  # Git for Windows wants C:/... rather than Git Bash's /c/... paths.
  if command -v cygpath >/dev/null 2>&1; then bundle="$(cygpath -m "$bundle")"; fi
  echo "$url can't be reached: using backup/craft-libs.bundle" >&2
  export CARGO_NET_GIT_FETCH_WITH_CLI=true
  export GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0="url.$bundle.insteadOf" GIT_CONFIG_VALUE_0="$url"
}

# What this build is made from, for the About section (upstream.txt).
upstream_summary() {
  echo "craft-suite $VERSION ($(git -C "$ROOT" rev-parse --short HEAD 2>/dev/null || echo unknown))"
  local name url branch sha
  while read -r name url branch; do
    case "$name" in '' | \#*) continue ;; esac
    sha="$(upstream_commit "$name")"
    echo "$name $(upstream_version "$name") ${sha:0:12} ($url $branch)"
  done < <(tr -d '\r' <"$ROOT/upstream.conf")
}
