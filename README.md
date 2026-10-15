# Craft Suite

Created by **Md. Hafijur Rahman**. Source: **ArtCraft**'s WordCraft, DeckCraft and GridCraft, and
**Markd**.

[WordCraft](https://github.com/storytold/wordcraft), [DeckCraft](https://github.com/storytold/deckcraft)
and [GridCraft](https://github.com/storytold/gridcraft) (open-source Rust reimplementations of Word,
PowerPoint and Excel by the ArtCraft team, shown in the suite as **Word**, **Slide** and
**Spreadsheet**, with the suite's own icons), and [Markd](https://github.com/Hafij-BGE/markd) (a
Markdown editor based on MarkText) in **one window with tabs**, packaged as **one `.deb`** (plus a
**Windows** installer and an **Android** app), kept up to date from the upstream repositories, with
room for your own changes.

- **Tabs that mix apps**: a Word document, a slide deck, a spreadsheet and a Markdown file side by
  side, each in its own tab with its app's icon and colour.
- **Markd tabs** for Markdown (`.md`): Markd's editor with its Word-style ribbon, themes, page
  layout and HTML, PDF and Word export ([below](#markd)).
- **Tab settings** (Home › Settings, or the ☰ button on the tab bar):
  - *arrangement*: tab bar at the top, bottom, left or right; tabs **mixed**, **grouped by app**, or
    in a **separate row per app**; the app order; where new tabs open; tab widths; icons; close
    buttons; pinned tabs as icons only;
  - *colour*: a colour per app (stripe, tint, solid or none), each tab's own colour (right-click a
    tab), a tinted tab bar, light/dark/system theme for the tab bar and pages;
  - *behaviour*: what a new tab shows, reusing untouched blank tabs, reopening last session's files,
    one window for files opened from the file manager, middle-click to close, keyboard shortcuts.
- **Drag tabs** to reorder them; right-click for pin, colour, move, close others, show in folder.
- **Ctrl+N** opens a new tab of the same app as the one you're in: a new document next to a
  document, a new spreadsheet next to a spreadsheet (Settings › Behaviour › Keyboard turns it off,
  giving Ctrl+N back to the app).
- **Tabs in the title bar** (the default): the tab bar is the window's title bar, with minimize,
  maximize and close beside the tabs; drag its empty part to move the window, double-click it to
  maximize, drag the window's edges to resize it. Settings › Window › Title bar switches back to
  the desktop's own title bar.
- **Home page**: blank document, presentation, spreadsheet and Markdown tiles, Open, and your recent files
  with search, a filter by app and when each file last changed (right-click one to show it in its
  folder or remove it from the list).
- **Updates**: a daily GitHub workflow merges new upstream commits; installed copies update with
  `craft-suite-update` or **Check for updates** on the Home page.
- **Ask AI** (desktop): an assistant beside every tab that answers from the document you're on and
  makes the changes you accept, live in the tab; through AI providers you choose, with your own
  free keys ([below](#ask-ai)).

## Layout

```
upstream/wordcraft/   WordCraft, as a git subtree (edit freely: see "Your own changes")
upstream/deckcraft/   DeckCraft, as a git subtree
upstream/gridcraft/   GridCraft, as a git subtree
upstream/markd/       Markd, as a git subtree
upstream.conf         which repositories and branches upstream/ follows
suite/                the Craft Suite app: the tabbed window that hosts all four
packaging/            build-deb.sh, build-windows.sh, build-apk.sh, build-markd-web.sh, desktop
                      entries, craft-suite-update, publish-release.sh
android/              the Android app: a WebView around the three apps' web versions
scripts/              update-upstream.sh, merge-locks.py, backup-deps.sh
backup/               copies of ArtCraft's craft-libs and craft-fonts, for when github.com/storytold
                      can't be reached
.github/workflows/    build-deb.yml (build + release), sync-upstream.yml (daily upstream merge)
```

The tabbed window (`suite/`) links the three apps' own UI libraries (`wordcraft-ui-egui`,
`deckcraft-ui-egui`, `gridcraft-ui-egui`) and runs each tab's app exactly as the app's own `main.rs`
does. The host helpers the upstream binaries use (file dialogs, clipboard, printing, audio) are
compiled straight from `upstream/*/apps/*/src`, so upstream fixes to them carry over.

Markd is a web app (TypeScript), not an egui one: a Markd tab shows the web version Markd's own
Android app runs (`upstream/markd/packages/android`, built by `packaging/build-markd-web.sh`) in
the system's web view, placed over the tab (`suite/src/markd/`). The page talks to the suite
through the same messages it sends Markd's Android app, so it needs no changes for the suite.

## Install

Works on 64-bit PCs (amd64) with Ubuntu 22.04 or newer, Debian 12 or newer, Linux Mint 21 or
newer, and similar distributions.

### From a terminal (with a token)

The lines below hold the update token, encrypted; you need its password. Paste them into a
terminal:

```sh
cd ~/Downloads
TOKEN="$(echo 'U2FsdGVkX186wTCYtCm9KNC5hOcBgdKJDXHYnDt10FC4F/LhnLDF2Ai0dAx+nNUbK2eEBAZ2YWnqLyorDvm9LPyWX7NvU7sjMPMxPBbok3O911prLnelXKVZep5NmKEWZgEPpc242VbT83tFSWHkZA==' | openssl enc -d -aes-256-cbc -pbkdf2 -iter 600000 -md sha256 -a -A)"
[ -n "$TOKEN" ] && mkdir -p ~/.config/craft-suite && (umask 077; printf 'TOKEN="%s"\n' "$TOKEN" >> ~/.config/craft-suite/update.conf)
```

The second line asks for the password (it doesn't show while you type) and unlocks the token.
The third saves it for this computer's updates, so **Check for updates** works from then on
(with a wrong password, openssl prints `bad decrypt` and nothing is saved: run both lines again).
Then paste these lines, as they are:

```sh
ID="$(curl -4 --http1.1 --retry 3 -fsSL -H "Authorization: Bearer $TOKEN" \
  https://api.github.com/repos/Hafij-BGE/software/releases/latest \
  | python3 -c 'import json, sys; print(next(a["id"] for a in json.load(sys.stdin)["assets"] if a["name"].endswith("_amd64.deb")))')"
curl -4 --http1.1 --retry 3 -fL -H "Authorization: Bearer $TOKEN" -H "Accept: application/octet-stream" \
  -o craft-suite_latest_amd64.deb "https://api.github.com/repos/Hafij-BGE/software/releases/assets/$ID"
unset TOKEN
ls -lh craft-suite_latest_amd64.deb       # about 110 MB; a tiny file is an error message
sudo apt install ./craft-suite_latest_amd64.deb
```

This always downloads the newest release. Notes:

- `curl: (22) … 401` means the token is wrong or expired; `404` means it can't read this
  repository. Make a new token.
- `curl: (35) TLS connect error` is a network problem, not the token: run the lines again. (`-4`
  and `--http1.1` are there because some networks break the connection without them.)
- apt may print `Notice: Download is performed unsandboxed as root … couldn't be accessed by user
  '_apt'`. That's harmless: apt reads the file from your home folder as root instead.

### From the browser

1. Open this repository's [**Releases**](https://github.com/Hafij-BGE/software/releases) page (log in to GitHub first: the
   repository is private) and, under **Assets** of the newest release, download
   `craft-suite_<version>_amd64.deb`, for example `craft-suite_0.1.0-r202610141015_amd64.deb`.
2. In a terminal, go to the folder the file was saved in (usually Downloads) and install it:

   ```sh
   cd ~/Downloads
   ls craft-suite_*.deb                      # check the file is there
   sudo apt install ./craft-suite_*_amd64.deb
   ```

   Keep the `./`: it tells apt to install that file. apt also installs the system libraries the
   suite needs. `Error: Unsupported file ./craft-suite_*_amd64.deb` means the file isn't in the
   folder you're in: `cd` to where it is (`find ~ -name 'craft-suite*.deb'` finds it).

A build from the **Actions** page (a run of "Build .deb" › **Artifacts** › `craft-suite-deb`)
downloads as a zip: `unzip craft-suite-deb.zip` first, then the same `apt install` line.

### After installing

Start **Craft Suite** from the applications menu (there are also Word, Slide, Spreadsheet and
Markd entries, which open a new tab of that app), or from a terminal:

```sh
craft-suite                       # the Home page (or last session's files)
craft-suite report.docx data.xlsx # each file in a tab of its app
craft-suite notes.md              # a Markd tab
craft-suite --new slide           # a blank presentation (--new mark: a blank Markdown file)
wordcraft-cli --help              # the headless converters/command runners are included
```

The package replaces the upstream `wordcraft`, `deckcraft` and `gridcraft` packages if they're
installed (it ships the same CLI tools, file types and icons).

## Update

- **From the app:** **Check for updates** on the Home page (or Settings › Updates) shows
  "Update available" when there's a newer release; **Install update** installs it after asking for
  your password. Settings › Starting › **Check for updates** checks each time the suite opens.
  Because this repository is private, this needs a read-only token on each computer, set up once:
  see [Updates from a private repository](#updates-from-a-private-repository).
- **From a terminal:** `craft-suite-update` (asks before installing), or
  `craft-suite-update --check` to only look.
- **By hand:** run the [terminal install](#from-a-terminal-with-a-token) lines again (they
  always fetch the newest release), or download the newer `.deb` from Releases and run the same
  `sudo apt install ./craft-suite_*_amd64.deb` in its folder. Settings and the files you had open
  are kept.

Restart Craft Suite after an update. Nothing is installed without your password.

## Windows

Every release also has a Windows (64-bit, Windows 10 or 11) build, the same suite as on Linux:

- `craft-suite_<version>_windows-x64-setup.exe`: the installer. It installs to Program Files (or
  just for you, if you choose), adds Craft Suite, WordCraft, DeckCraft and GridCraft to the Start
  menu, offers a desktop shortcut, and lists Craft Suite under **Open with** for documents,
  presentations and spreadsheets (without taking over your defaults). Uninstall it in
  Settings › Apps.
- `craft-suite_<version>_windows-x64.zip`: no installer; unzip it anywhere and run
  `craft-suite.exe`.

The files aren't code-signed, so Windows SmartScreen warns about an unknown publisher the first
time: **More info › Run anyway**. Settings are kept in `%APPDATA%\craft-suite`. On Windows, for now,
each start opens its own window (no handing files to an open window), and **Get updates** on the
Home page opens the Releases page, where you download the newer installer and run it over the old
one.

## Markd

Markdown files (`.md`, `.markdown`) open in **Markd** tabs; **Blank markdown** on the Home page (or
`craft-suite --new mark`, or Ctrl+N in a Markd tab) starts one. A Markd tab is Markd's editor:

- WYSIWYG Markdown (MarkText's muya editor) with a Word-style ribbon: **File, Home, Insert, Draw,
  Layout, View**; Source view for the raw Markdown; find and replace; word and page counts.
- Page layout on A4, US Letter or Legal paper; Word Light/Dark themes that follow the system.
- **Export**: HTML, **PDF** (written straight to the file you choose) and **Word (.docx)**.
- Pictures you paste or insert, and sketches (Draw), are saved in an `assets` folder next to the
  `.md` file.
- The ☰ menu: New, Open, **Open folder** (a folder's Markdown files) and Markd's own recent files.
- Keys: **Ctrl+S** save, **Ctrl+Shift+S** save as, **Ctrl+O** open, **Ctrl+F** find, **Ctrl+H**
  replace, **Ctrl+P** export PDF; the suite's tab keys (Ctrl+N, Ctrl+Alt+PageUp/PageDown…) work
  in the page too.
- Closing a tab with unsaved changes asks in Markd (Save, Discard, or Cancel by dismissing it).
  Markd's **AutoSave** (on by default, File tab) saves a file that has a name as you type.

Good to know:

- Markd tabs need the system web view: **WebKitGTK 4.1** on Linux (`libwebkit2gtk-4.1-0`, which
  the package depends on) and **WebView2** on Windows 10/11 (built in). On a Wayland desktop the
  suite runs through XWayland, as it already does for drag and drop; with `CRAFT_SUITE_WAYLAND=1`
  Markd tabs don't open.
- The page is a native window over the tab: while a menu or prompt of the suite is open over it,
  the page steps aside.
- WordCraft still opens Markdown from its own File › Open.
- Markd's settings and recent files are kept in `~/.local/share/craft-suite/markd` (Windows:
  `%LOCALAPPDATA%\craft-suite\markd`).
- Ask AI in a Markd tab sees the file's name only, not its text.
- To build the suite without Markd (no WebKitGTK needed):
  `cargo build --release --no-default-features --features ai` in `suite/`.
- On Windows, **Export PDF** opens the page in your browser, to print it to PDF from there.

## Ask AI

Click **Ask AI** in the tab bar: a panel opens beside the tab. Each tab has its own conversation
and the AI sees only that tab (the Home page's conversation sees no document).

- **Answers from your document**: summarise, explain, improve the selected text, suggest a chart…
  Suggested questions show for each kind of tab; **Copy**, **Stop** and **New chat** are there.
- **Changes you can see**: when you ask for a change, the answer lists the steps (insert text,
  apply a style, add slides, fill cells, make a chart…). **Apply** makes them in the tab at once;
  **Ctrl+Z** undoes them. The AI can only use a short list of editing commands: nothing that opens,
  saves, prints or deletes files.
- **Data analyser**: **Attach CSV…** (any tab) or **Analyse sheet** (a spreadsheet tab). Craft Suite
  computes the statistics itself, per column n, missing, mean ± SD, median (IQR), min–max, group
  comparisons and correlations, shows them as a table, and sends **only these numbers** to the AI,
  never the rows. The AI is told to explain them and never calculate.
- **Private** (on by default, per chat): the AI sees your question, the headings or slide titles,
  the selected text and the computed statistics. Off: also the full text, speaker notes and the
  sheet's first rows (up to a size limit). Every question shows what was shared.
- The AI is told to answer only from what it's given, to say when something is missing, and never
  to invent numbers, dates, citations or names. Answers show as plain formatted text; only
  http, https and mailto links are clickable.

### Adding your keys

Settings › **Ask AI** › *Add a channel*: OpenRouter, Google Gemini, Groq or Mistral. Each channel
has a **Get a free key** link to the provider's key page; paste the key into the app (**never into
a chat, an issue or a file in this repository**), then **Find models** and **Test this channel**.

- Keys are stored in your system keyring (GNOME/KDE's Secret Service on Linux, Credential Manager on
  Windows), or, without one, in `~/.config/craft-suite/ai-keys.json` readable only by you. Never in
  the settings, the program, the `.deb` or this repository. Each key is sent only to its own
  provider, straight from your computer.
- Keys don't sync between computers: add them on each one.

### How switching works

Channels are tried in their order (↑/↓). The first that can answer does. On a rate limit (429),
daily limit, used-up quota, refused key or server error, the question goes to the next channel and
the answer says why it switched ("Switched channels"). A channel that hit a limit **rests**:

- per-minute limits: for the time the provider says (Retry-After), else a minute;
- daily limits: until the limit resets, midnight UTC (Google Gemini: midnight Pacific time);
- OpenRouter's daily free-request limit counts for all its free models together; Craft Suite asks
  OpenRouter how many free requests are left to tell it from a per-minute limit;
- a refused key: until you change the key or test the channel again.

Settings shows each channel as **Ready** or **Resting until …**. OpenRouter's model **auto** reads
its free models live and tries the strongest first (by OpenRouter's intelligence index where it
lists one, else by size), up to four per question.

### Limits

- Free models are weaker than paid ones, and free limits are low (as of October 2026, OpenRouter's
  free models allow 50 requests a day, 1000 after buying 10 credits; Google, Groq and Mistral show
  your limits in their consoles). Check the providers' pages: limits change.
- Free plans may use what you send to improve their models: Google's terms say so for unpaid use,
  Mistral's free mode does unless you opt out (Admin › Privacy), and OpenRouter's free models depend
  on the provider behind each one. Keep Private on for anything sensitive.
- Ask AI needs an internet connection; it is in the desktop suite (Linux, Windows), not the Android
  app.

### Turning it off, or removing it

- **Off:** Settings › Ask AI › *Turn on Ask AI*. No button, no panel, nothing is sent. Channels and
  keys stay for when you turn it back on.
- **Delete your data:** Settings › Ask AI › *Remove all AI data…* deletes every key (from the
  keyring too), channel and chat.
- **Remove it from the program:** everything is in `suite/src/ai/`, behind the `ai` Cargo feature
  (on by default). Build without it with `cargo build --release --no-default-features` in `suite/`,
  or remove `"ai"` from `default` in `suite/Cargo.toml`; to delete it for good, delete
  `suite/src/ai/` and the lines marked `#[cfg(feature = "ai")]`.

## Android

Every release also has `craft-suite_<version>.apk` (Android 8.0 or newer): a small app that shows
WordCraft, DeckCraft and GridCraft's **web versions** (the same apps, compiled to WebAssembly) from
inside the app, so it works offline. Its first page has New Document, Presentation and Spreadsheet,
**Open a file** and your **Recent** files; **Back** returns there. (Markd isn't part of the Android
app: Markd has its own APK.)

- **Install:** download the `.apk` on the phone (logged in to GitHub: the repository is private),
  open it, and allow installing apps from your browser or file manager when Android asks.
- **Opening and saving:** **Open a file** (or File › Open in each app) picks a file from the
  phone; saving asks where to put the file (the system's Save screen).
- **Open with:** documents, presentations and spreadsheets in your file manager, mail or chat apps
  list **Craft Suite** (to edit) and **Craft Suite Viewer** (to read) under *Open with* and
  *Share*: tap the file, pick one, and it opens in the right app.
- **Read-only:** Craft Suite Viewer, or the **Read-only** switch on the first page (for Open a file
  and Recent), opens a file to read: a document in Read Mode, a presentation as a slide show, a
  spreadsheet as it is; typing, the on-screen keyboard and saving are off, and the file itself is
  never changed. **Edit** (bottom left) opens the same file again, editable.
- **Touch:** drag with **two fingers** to scroll, **pinch** to zoom; one finger selects and taps as
  before. Scroll bars are wider and always shown, so a finger can grab them.
- **Stylus:** a pen writes and selects at once, and a hand resting on the screen is ignored while
  the pen is down.
- **Not the desktop suite:** one document at a time, no tabs, and the apps' desktop layout (made
  for a mouse and a wide screen: a tablet, or a phone held sideways, works best). Printing, the
  spelling dictionary and other desktop-only parts depend on what each web version supports.
- **Updates:** **Check for updates** on the app's first page opens the newest release in your
  browser; download its `.apk` and open it, and it installs over the old one, keeping your files.
  Android only allows that when both are signed with the same key, so the repository needs a
  signing key of its own (below); until then each build has a new key, and an update needs
  uninstalling the old app first.

### The Android signing key (once)

On your Linux computer, in this repository's folder (or download just
`packaging/make-android-key.sh`):

```sh
sh packaging/make-android-key.sh
```

It makes the key in `~/craft-suite-android-key` (only `openssl` needed) and prints what to do
next: on GitHub, Settings › Secrets and variables › Actions › **New repository secret**, twice:

- `ANDROID_KEYSTORE_BASE64`: the contents of `~/craft-suite-android-key/craft-suite.p12.b64`
- `ANDROID_KEYSTORE_PASSWORD`: the line in `~/craft-suite-android-key/password.txt`

From the next build on, every APK is signed with that key, so each one installs over the last as
an update. Keep `craft-suite.p12` and `password.txt` somewhere safe and private (not in the
repository): with a different key, installed apps can't be updated. An APK built before the key
was added needs uninstalling once.

## Uninstall

```sh
sudo apt remove craft-suite     # the program, menu entries, file types and icons
sudo apt purge craft-suite      # the same, and /etc/craft-suite/update.conf
```

Your settings stay in your home folder, in case you reinstall. To delete them too:
`rm -rf ~/.config/craft-suite` (the suite's) and, only if you don't also use the standalone apps,
`~/.config/wordcraft ~/.config/deckcraft ~/.config/gridcraft`. Your documents are never touched.

## Build it yourself

On Ubuntu 22.04 or newer (or Debian 12+):

```sh
sudo apt install build-essential pkg-config git curl python3 libasound2-dev libxkbcommon-dev \
  libwayland-dev libx11-dev libxcursor-dev libxrandr-dev libxi-dev libgl1-mesa-dev \
  libfontconfig1-dev libfreetype-dev libwebkit2gtk-4.1-dev libgtk-3-dev nodejs
curl https://sh.rustup.rs -sSf | sh            # Rust 1.90 or newer
corepack enable                                # pnpm, for Markd (Node.js 20.19 or newer)
packaging/build-deb.sh --with-fonts            # -> dist/craft-suite_0.1.0-r<time>_amd64.deb
```

`--with-fonts` embeds the fonts the official releases embed (including the Japanese and Chinese
interface fonts) from [craft-fonts](https://github.com/storytold/craft-fonts); leave it out for a
quicker build with egui's default fonts. A first build takes a while (three office apps).

To try the window without packaging: `packaging/build-markd-web.sh` once (Markd's web files, in
`build/markd/www`), then `cd suite && cargo run --release`.

**Windows:** in Git Bash, with Rust (`rustup`, MSVC), Node.js with pnpm and, for the installer,
[Inno Setup 6](https://jrsoftware.org/isinfo.php): `packaging/build-windows.sh --with-fonts`
(-> `dist/craft-suite_<version>_windows-x64.zip` and `-setup.exe`).

**Android:** with Rust's `wasm32-unknown-unknown` target, [trunk](https://trunkrs.dev), a JDK and
the Android SDK (`ANDROID_HOME`, a platform and build-tools): `packaging/build-apk.sh`
(-> `dist/craft-suite_<version>.apk`; set `ANDROID_KEYSTORE` and `ANDROID_KEYSTORE_PASSWORD` to sign
it with your key).

## Updates from upstream

```sh
scripts/update-upstream.sh --check   # which apps have new upstream commits
scripts/update-upstream.sh           # merge them (all apps, or name some: wordcraft gridcraft)
packaging/build-deb.sh               # build the new package
```

### What happens automatically

1. **Every day at 03:17 UTC**, `.github/workflows/sync-upstream.yml` checks WordCraft, DeckCraft and
   GridCraft for new commits. With nothing new, it stops there.
2. With new commits, it merges them into `upstream/` (keeping your own changes), refreshes
   `backup/` (below) and checks that the suite still compiles.
3. **A clean update that compiles is merged into `main` and released automatically**: the Linux
   `.deb`, the Windows installer and the Android app (`.github/workflows/build-deb.yml`, about 40
   minutes). Nothing to click.
4. An update that conflicts with your own changes, or doesn't compile, comes as a pull request
   from the branch `upstream-sync` instead (with the packages built on it), so nothing broken is
   released; merging it releases it.
5. Installed copies find the release with **Check for updates** (above).

Any push to `main` (a merged pull request too) builds and publishes a release. To review every
ArtCraft update before it's released, set the repository variable **`AUTO_UPDATE`** to `false`
(Settings › Secrets and variables › Actions › Variables › New repository variable): each update
then comes as a pull request.

### If ArtCraft's repositories disappear

Everything needed to keep building is in this repository:

- `upstream/`: the full source of WordCraft, DeckCraft and GridCraft (git subtrees, with your
  changes).
- `backup/craft-libs.bundle`: the craft-libs library the apps build with (one git bundle file,
  with its full history and every commit the `Cargo.lock` files pin).
- `backup/craft-fonts/<commit>/`: the fonts each app's release embeds (Japanese, Chinese, Arabic
  and the rest).

Builds use github.com/storytold as usual; when it can't be reached, `packaging/lib.sh` takes these
copies instead (the same commits, so nothing else changes), and says so in the build log. The
daily sync then just warns that there's nothing to fetch. `scripts/backup-deps.sh` refreshes the
copies, and the sync runs it after each update, so they follow the versions the apps use. The
other libraries come from crates.io, which keeps every published version.

Releases are named after the time of the commit they're built from:
`Craft Suite 0.1.0+r202610141015` is 2026-10-14 10:15 UTC, so a newer build always has a higher
version. (The file names spell the `+` as `-`.) `VERSION` holds the `0.1.0` part.

Repository settings this needs:

- Settings › General › **Default branch: `main`** (releases are published from it).
- Settings › Actions › General › Workflow permissions: **Read and write**, and
  **Allow GitHub Actions to create and approve pull requests** (for the sync pull requests).
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

The app icons are drawn by `scripts/make-app-icons.py` (into `assets/hicolor/`).

Changes to the suite itself (the tab bar, settings, Home page) go in `suite/src/`; how a Markd
tab hosts its page is `suite/src/markd/`.

The Android app's changes to the apps (wider scroll bars, the commands its read-only mode runs) are
patches in `android/patches/`, applied to `upstream/` only while `packaging/build-apk.sh` builds
the web apps and undone afterwards, so they never conflict with upstream updates. A patch an update
no longer fits is left out with a warning (the APK still builds, without that change).

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
apps; Markd is MIT (`upstream/markd/LICENSE`). Each app's licence and attribution files are in its `upstream/<app>/` folder and are
installed to `/usr/share/doc/craft-suite/`.
