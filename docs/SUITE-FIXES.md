# Suite fixes to keep

Fixes the suite made on top of WordCraft, DeckCraft and GridCraft. An upstream sync
(`scripts/update-upstream.sh`) or a port to a new upstream API can undo any of them, so
`scripts/check-fixes.sh` checks each one. `scripts/preflight.sh` runs it, so every `git push` does too.

When you add a fix, add it here and add its check line to `scripts/check-fixes.sh`.
If a check fails after a sync, port the fix to the new code. Never delete the check to make it pass.

| # | Fix | Where | Commit / PR |
|---|-----|-------|-------------|
| 1 | The suite's interface size (Settings ▸ size, `ui_size`, `same_size`) always wins over an app's own size. Before this, WordCraft set its own zoom every frame and the window shook. The suite size goes into WordCraft's `FrameInput.root.scale`. `pinned_zoom` wraps every app call. A warning is logged if an app still changes the zoom. | `suite/src/hosts.rs`, `suite/src/app.rs` | 34c2fa0aaf (#32) |
| 2 | WordCraft's craft runtime is installed once, when the window is created, not in the middle of a frame. Otherwise a debug assertion fires: "runtime plugin's hooks did not run". | `suite/src/main.rs`, `suite/src/hosts.rs` | #31 |
| 3 | Every app's top row is 38 points, like Slides. WordCraft's row is forced with the `size.bar.title` size override. | `suite/src/hosts.rs` `WordTab::suite_sizes`, `suite/src/tabbar.rs` `APP_TITLE_H` | b8b3bb22d0 (#34) |
| 4 | The in-app minimize, maximize and close buttons are drawn in each app's top row, at that row's height. They never cover Comments or Share, and they are never removed. | `suite/src/tabbar.rs` `app_window_controls`, `suite/src/app.rs` | 48a9dd5d57 (#34) |
| 5 | Each app leaves room at the end of its title bar for those buttons (`title_bar_end`). | `upstream/*/…` `title_bar_end`, `suite/src/hosts.rs` `set_title_bar_end` | 52d04311ab (#27) |
| 6 | WordCraft's AutoSave switch is centred in the top row, like the Quick Access buttons beside it. It is no longer stuck to the top edge. | `upstream/wordcraft/crates/ui-egui/src/chrome/title_bar.rs` `autosave` | word-autosave-row |
| 7 | Slideshows fill the screen: the tab bar, the AI panel and the font bar hide while a slideshow runs. | `suite/src/app.rs` `presenting` | 97904dccc8 |
| 8 | Shared Stock Images, icons and page designs, and the extra DeckCraft shapes. | `upstream/*/…`, `upstream/deckcraft/crates/engine/src/suite_presets.rs` | 0f505b8659 (#29) |
| 9 | Screenshot hook used to test UI fixes. `CRAFT_SUITE_SCREENSHOT=<png>` (and optionally `CRAFT_SUITE_SCREENSHOT_FRAMES`) saves the window and quits. | `suite/src/shot.rs` | b8b3bb22d0 (#34) |
| 10 | Lock files match `--locked` builds, and the apps and the suite share one revision of each fork pin. | checked by `scripts/preflight.sh` itself | c5360cfba7 (#26) |
| 11 | Word's Home ▸ Font row keeps a 4-point gap between the font box and the size box. They no longer run into each other when the missing-font sign shows. | `upstream/wordcraft/crates/ui-egui/src/ribbon/home.rs` `FONT_BOX_GAP` | word-font-gap |

## Checking a UI fix

Look before you merge. Build the suite, then run it in a headless GNOME shell with the owner's
settings (title bar in the app, `same_size` on, `ui_size` 118). Set `CRAFT_SUITE_SCREENSHOT`.
Compare the Word, Slide and Sheet tabs.
