# Craft Suite

[WordCraft](https://github.com/storytold/wordcraft), [DeckCraft](https://github.com/storytold/deckcraft)
and [GridCraft](https://github.com/storytold/gridcraft) (open-source Rust reimplementations of Word,
PowerPoint and Excel by the ArtCraft team) in **one window with tabs**, packaged as **one `.deb`**,
kept up to date from the upstream repositories, with room for your own changes.

- **Tabs that mix apps**: a Word document, a slide deck and a spreadsheet side by side, each in its
  own tab with its app's icon and colour.
- **Tab settings** (Home › Settings, or the ☰ button on the tab bar):
  - *arrangement*: tab bar at the top, bottom, left or right; tabs **mixed**, **grouped by app**, or
    in a **separate row per app**; the app order; where new tabs open; tab widths; icons; close
    buttons; pinned tabs as icons only;
  - *colour*: a colour per app (stripe, tint, solid or none), each tab's own colour (right-click a
    tab), a tinted tab bar, light/dark/system theme for the tab bar and pages;
  - *behaviour*: what a new tab shows, reusing untouched blank tabs, reopening last session's files,
    one window for files opened from the file manager, middle-click to close, keyboard shortcuts.
- **Drag tabs** to reorder them; right-click for pin, colour, move, close others, show in folder.
- **Updates**: a daily GitHub workflow merges new upstream commits; installed copies update with
  `craft-suite-update` or **Check for updates** on the Home page.

## Layout

```
upstream/wordcraft/   WordCraft, as a git subtree (edit freely: see "Your own changes")
upstream/deckcraft/   DeckCraft, as a git subtree
upstream/gridcraft/   GridCraft, as a git subtree
upstream.conf         which repositories and branches upstream/ follows
suite/                the Craft Suite app: the tabbed window that hosts all three
packaging/            build-deb.sh, desktop entries, craft-suite-update
scripts/              update-upstream.sh, merge-locks.py
.github/workflows/    build-deb.yml (build + release), sync-upstream.yml (daily upstream merge)
```

The tabbed window (`suite/`) links the three apps' own UI libraries (`wordcraft-ui-egui`,
`deckcraft-ui-egui`, `gridcraft-ui-egui`) and runs each tab's app exactly as the app's own `main.rs`
does. The host helpers the upstream binaries use (file dialogs, clipboard, printing, audio) are
compiled straight from `upstream/*/apps/*/src`, so upstream fixes to them carry over.

## Install

Download `craft-suite_<version>_amd64.deb` from this repository's Releases page, then:

```sh
sudo apt install ./craft-suite_*_amd64.deb
```

Start **Craft Suite** from the applications menu (there are also WordCraft, DeckCraft and GridCraft
entries, which open a new tab of that app), or from a terminal:

```sh
craft-suite                       # the Home page (or last session's files)
craft-suite report.docx data.xlsx # each file in a tab of its app
craft-suite --new deck            # a blank presentation
wordcraft-cli --help              # the headless converters/command runners are included
```

The package replaces the upstream `wordcraft`, `deckcraft` and `gridcraft` packages if they're
installed (it ships the same CLI tools, file types and icons).

## Build it yourself

On Ubuntu 22.04 or newer (or Debian 12+):

```sh
sudo apt install build-essential pkg-config git curl python3 libasound2-dev libxkbcommon-dev \
  libwayland-dev libx11-dev libxcursor-dev libxrandr-dev libxi-dev libgl1-mesa-dev \
  libfontconfig1-dev libfreetype-dev
curl https://sh.rustup.rs -sSf | sh            # Rust 1.90 or newer
packaging/build-deb.sh --with-fonts            # -> dist/craft-suite_<version>_<arch>.deb
```

`--with-fonts` embeds the fonts the official releases embed (including the Japanese and Chinese
interface fonts) from [craft-fonts](https://github.com/storytold/craft-fonts); leave it out for a
quicker build with egui's default fonts. A first build takes a while (three office apps).

To try the window without packaging: `cd suite && cargo run --release`.

## Updates from upstream

```sh
scripts/update-upstream.sh --check   # which apps have new upstream commits
scripts/update-upstream.sh           # merge them (all apps, or name some: wordcraft gridcraft)
packaging/build-deb.sh               # build the new package
```

On GitHub, `.github/workflows/sync-upstream.yml` does this every day: it merges the upstream changes
on a branch, checks that the suite still compiles, and opens a pull request; merging it builds and
publishes a new release, which installed copies then find. Set the repository variable
`AUTO_UPDATE` to `true` to skip the pull request when the merge is clean and compiles. Two
repository settings it needs:

- Settings › Actions › General › Workflow permissions: **Read and write**, and
  **Allow GitHub Actions to create and approve pull requests** (for the pull requests).
- The update check on installed copies reads the newest release. For a **public** repository
  that needs nothing more. For a **private** one, give each computer a read-only token (below).

### Updates from a private repository

1. On GitHub: your picture › **Settings › Developer settings › Personal access tokens ›
   Fine-grained tokens › Generate new token**. Pick an expiry date; under **Repository access**
   choose **Only select repositories** and this repository; under **Permissions › Repository
   permissions** set **Contents** to **Read-only**. Generate it and copy it (`github_pat_...`).
2. On each computer with Craft Suite (as yourself, not with `sudo`):

   ```sh
   mkdir -p ~/.config/craft-suite
   echo 'TOKEN="github_pat_...paste it here..."' >> ~/.config/craft-suite/update.conf
   chmod 600 ~/.config/craft-suite/update.conf
   ```

3. Then **Check for updates** on the Home page (or in Settings), or `craft-suite-update` in a
   terminal, finds and installs new releases; it asks for your password to install.

The token can only read this repository. It is stored in that file as plain text, readable only by
you (`chmod 600`); when it expires or you revoke it on GitHub, make a new one and replace the line.

## Your own changes

Everything under `upstream/` is ordinary files in this repository: change anything, in any app, and
commit. `scripts/update-upstream.sh` merges upstream's new commits *with* your commits (`git subtree
pull`), like merging any branch: your changes stay. When you and upstream changed the same lines,
git stops with a conflict to resolve (`git status`, edit, `git add`, `git commit`); the daily
workflow opens a pull request with the conflict instead.

Changes to the suite itself (the tab bar, settings, Home page) go in `suite/src/`.

`upstream.conf` names the repositories and branches; point an app at your own fork there (then
`git subtree pull` from it once by hand, or let the script do it on the next run).

## Good to know

- Only the tab on show runs. A tab in the background keeps its document but pauses: autosave and
  anything else the app does on its own continue when you go back to it.
- Each app keeps its own options (File › Options in the app), in its usual settings folder
  (`~/.config/wordcraft`, `~/.config/deckcraft`, `~/.config/gridcraft`), shared with the
  standalone apps. With several tabs of one app, the options of the one used last are saved.
- DeckCraft's AutoRecover is off inside the suite: it keeps one recovery folder per user, and
  several DeckCraft tabs would recover or clear each other's files. Save your presentations.
- The apps' agent control servers (`--control`) and macOS-specific integrations aren't part of the
  suite window; the `*-cli` tools are.
- Each app draws its own ribbon and title area inside its tab, so some window-level actions (for
  example the title-bar drag area some apps draw) behave as in the standalone app.
- `CRAFT_SUITE_NO_PREFS=1 craft-suite` runs without reading or writing any settings.

## Licences

The suite code (`suite/`, `packaging/`, `scripts/`, `assets/`) is MIT OR Apache-2.0, like the
apps. Each app's licence and attribution files are in its `upstream/<app>/` folder and are
installed to `/usr/share/doc/craft-suite/`.
