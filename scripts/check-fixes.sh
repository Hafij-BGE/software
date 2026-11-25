#!/usr/bin/env bash
# Checks that the suite's own fixes (docs/SUITE-FIXES.md) are still in the code. Run it after every
# upstream sync; scripts/preflight.sh runs it, so every `git push` does too.
#
# Each check is: fix number | file (a glob is fine) | text that must be in that file.
# A failed check means a sync or port dropped the fix: port the fix again, never delete the check.
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

CHECKS='
1|suite/src/hosts.rs|input.root.scale = craft::ui::runtime::env::Scale::new(zoom)
1|suite/src/app.rs|fn pinned_zoom
1|suite/src/app.rs|set its own interface size
2|suite/src/main.rs|hosts::word_install_runtime(&cc.egui_ctx)
3|suite/src/tabbar.rs|pub const APP_TITLE_H: f32 = 38.0;
3|suite/src/hosts.rs|"size.bar.title"
3|suite/src/hosts.rs|fn suite_sizes
4|suite/src/tabbar.rs|pub fn app_window_controls
4|suite/src/app.rs|app_window_controls(ui.ctx(), app_area, title_h)
4|suite/src/hosts.rs|fn title_bar_height
5|suite/src/hosts.rs|fn set_title_bar_end
5|upstream/wordcraft/crates/ui-egui/src/lib.rs|pub title_bar_end: f32
5|upstream/deckcraft/crates/ui-egui/src/lib.rs|pub title_bar_end: f32
5|upstream/gridcraft/crates/ui-egui/src/services.rs|pub title_bar_end: f32
6|upstream/wordcraft/crates/ui-egui/src/chrome/title_bar.rs|Craft Suite: centred in the bar'"'"'s height
7|suite/src/app.rs|fn presenting
7|suite/src/hosts.rs|pub fn presenting
8|upstream/deckcraft/crates/engine/src/suite_presets.rs|Craft Suite
8|upstream/wordcraft/crates/ui-egui/src/lib.rs|Stock Images and Online Pictures
11|upstream/wordcraft/crates/ui-egui/src/ribbon/home.rs|ui.add_space(FONT_BOX_GAP);
9|suite/src/main.rs|mod shot;
9|suite/src/shot.rs|CRAFT_SUITE_SCREENSHOT
'

fail=0
n=0
echo "==> suite fixes (docs/SUITE-FIXES.md)"
while IFS='|' read -r fix file text; do
  [ -z "$fix" ] && continue
  n=$((n + 1))
  # shellcheck disable=SC2086 # the file field may be a glob
  if ! grep -qF -- "$text" $file 2>/dev/null; then
    echo "  FAIL  fix $fix: \"$text\" is missing from $file"
    fail=1
  fi
done <<<"$CHECKS"

if [ "$fail" = 1 ]; then
  echo "        fix: port each missing fix again (see docs/SUITE-FIXES.md); never drop the check" >&2
  exit 1
fi
echo "  ok    $n checks, every fix in place"
