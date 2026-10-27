# Markd for Android

Android app for Markd. A small native shell (Java, one `WebView`) hosts the
standalone `@muyajs/core` editor from `packages/muya`. The Electron renderer in
`packages/desktop` is not used, because it depends on Electron IPC. A native
home screen and the Inkwell drawing / PDF-annotation canvas
(`android/inkwell`) sit around the editor.

## Features

- WYSIWYG Markdown editing (muya), with a touch-friendly Word-style ribbon:
  **File, Home, Insert, Draw, Layout, View**. The ribbon scrolls sideways on
  small screens and collapses when you tap the active tab.
- Word Light / Word Dark themes, imported from the desktop app's theme CSS. They follow
  the system theme by default.
- Print layout on A4 / US Letter / US Legal paper with Word margins. It fits the
  page width or shows it at 100%. Mobile view lets text reflow, and Source shows
  the raw Markdown.
- Files through the Storage Access Framework: open and save `.md` files, open
  whole folders, keep a recent-files list, and handle "Open with" from other
  apps. AutoSave writes open files after each pause in typing and when the app
  goes to the background. Unsaved untitled documents are kept as a draft.
- Export:
  - **HTML**: a self-contained, themed page.
  - **PDF**: opens the Android print dialog. Choose "Save as PDF".
  - **Word (.docx)**: uses `@turbodocx/html-to-docx` inside the WebView, with
    the same computed-style inlining as the desktop exporter.
- **Home screen** (the launcher; File → *Home* in the editor returns to it).
  It shows one folder you pick once, e.g. `Documents/Markd`:
  - a grid of Markdown files, drawings and PDFs with previews and badges
    (`MD`, `INK`, `PDF · 2 ink`), plus subfolders, search, sort and pins;
  - a *Recent* row with what you opened last, including Markdown from other
    places;
  - **+** for New Markdown, New drawing, Annotate a PDF, Import file and New
    folder. Long-press any file to rename, duplicate, share or delete it.

  Markdown opens in the editor. Drawings and PDFs open in the Inkwell canvas.
- **Drawings and PDF annotation** (Inkwell, modelled on Xournal++):
  - pen with pressure, five widths and solid/dash/dash-dot/dot lines;
    highlighter; eraser (touched part or whole stroke); text; images;
    line, rectangle, ellipse, arrows and axes; shape recogniser; lasso and
    rectangle selection (move, resize, duplicate, copy/cut/paste, recolour);
    pages, layers, undo/redo and stylus-only mode;
  - **opacity**: the toolbar shows the pen, shape or highlighter opacity
    (e.g. `50%`); tap it for a 5–100% slider. Long-press the fill button for
    fill opacity.
  - Drawings are `.ink` files in the Xournal++ file structure. Rename one to
    `.xopp` to open it in desktop Xournal++; desktop `.xopp` files import.
  - **A PDF is never copied or changed.** Its annotation is an `.ink` file that
    links to the PDF where it is. Tapping a PDF opens its annotation, starts one
    if there is none, or lists several to open or delete (*New annotation*
    adds another).
  - *Export PDF to folder* writes `<note>.pdf` (`<note> (annotated).pdf` for a
    PDF) next to the note: a vector PDF with the ink drawn in, so the original
    text stays selectable.
- Stylus in the Markdown editor:
  - **Pen handwriting to text** (Android 14+). Write on the page with the pen
    and Android turns the writing into text at that spot. Draw → *Pen to text*
    turns this on or off (`View.setAutoHandwritingEnabled`).
  - **Sketch** (Insert/Draw → *Sketch*) opens the Inkwell canvas and inserts an
    SVG that carries the whole drawing; tap the image later for *Edit in
    Inkwell*.
  - **Quick sketch** (Draw) is the simple pad: pressure-sensitive ink, a
    highlighter and a stroke eraser. Tilting the pen gives broader strokes, and
    hovering shows a preview of the tip.
  - Sketches are saved in `assets/` next to the `.md` file (the home screen
    hides that folder). If the app cannot reach the folder, the image is
    embedded in the document.
  - **Palm rejection** while a pen is in use; the pen's eraser end or side
    button erases strokes; the status bar shows pen hover, pressure and tilt.

## Files in the home folder

| File | What it is |
| --- | --- |
| `*.md` | Markdown, edited in the Markd editor |
| `*.ink` | A drawing or a PDF annotation (gzip XML, Xournal++ fileversion 4) |
| `*.pdf` | PDFs you keep there, and PDFs exported from notes |
| `assets/` | Images inserted into Markdown files |
| `*.ink.bg.pdf` | Attached PDF copy from 0.24.0 notes; still read, no longer written |

A PDF annotation stores the PDF's real path as Xournal++'s absolute
`filename` (e.g. `/storage/emulated/0/Download/X.pdf`) and its content URI in
an `inkwell-uri` attribute on the first PDF `<background>`. To open it, Markd
tries the URI, then the path, then a PDF with the same name next to the note
and anywhere in the folder, and only then asks you to choose it; choosing it
relinks every note in that folder that used the same PDF.

Android forgets an app's URI grants when it is uninstalled. Opening a PDF by
its path needs *All files access* (asked for when needed, or Home → ⋮ → *Allow
all files access*), so after a reinstall linked PDFs are found again unless
they were moved.

## How it fits together

```
src/                 web app (TypeScript, Vite) → android/app/src/main/assets/www
  editor.ts          muya setup; maps relative image paths to the /doc/ route
  ribbon.ts          ribbon tabs and groups
  document.ts        open / save / recent files / image assets (via native.ts)
  ink.ts, pen.ts     quick sketch pad and stylus status
  inkwell.ts         Sketch through Inkwell, Edit in Inkwell, File → Home
  export.ts          HTML / PDF / DOCX
android/             Gradle project
  MainActivity.java  WebView host, serves https://appassets.androidplatform.net
  Bridge.java        JS ⇄ native calls (SAF pickers, file IO, printing)
  DocStore.java      Storage Access Framework helpers
android/inkwell/     Inkwell library module (Java, PdfBox-Android)
  HomeActivity       the launcher: folder grid, Recent row, + menu
  EditorActivity     drawing / PDF canvas; SketchActivity for Markdown sketches
  io/, pdf/, model/  .ink reader/writer, SVG sketches, vector PDF export
```

The page is served from `https://appassets.androidplatform.net`:

- `/assets/www/…` comes from the APK.
- `/doc/<token>/<path>` comes from the folder of the open document, so Markdown
  such as `![](assets/sketch.png)` displays.

The JS bridge uses `WebViewCompat.addWebMessageListener`. It is limited to that
origin and answers only the main frame, so HTML or iframes inside a document
cannot call the file APIs.

### Images next to the document

With Android's storage model, opening a single file grants access to that file
only. To read or write `assets/…` beside it, Markd asks you once to pick the
folder that contains the document, and keeps that grant. If you open files from
a folder added with **☰ → Open folder**, no extra step is needed.

## Requirements

- Node.js 22+ and pnpm 10 (from the repository root: `corepack enable`).
- JDK 17. Newer JDKs also work with AGP 8.7, but 17 is what CI uses.
- Android SDK with **Platform 35** and **Build-Tools 35**. Install them with
  Android Studio, or with the command-line tools:

  ```bash
  sdkmanager "platforms;android-35" "build-tools;35.0.0" "platform-tools"
  ```

  Then point Gradle at the SDK. Either set `ANDROID_HOME`, or create
  `android/local.properties` containing `sdk.dir=/path/to/Android/Sdk`.
- The Gradle wrapper downloads Gradle 8.11.1 on the first run. The Android
  Gradle Plugin is 8.7.3, compileSdk and targetSdk are 35, and minSdk is 26
  (Android 8.0).

## Build

From the repository root:

```bash
pnpm install
pnpm --filter @markd/android build:apk
```

The APK is written to
`packages/android/android/app/build/outputs/apk/debug/Markd-<version>-android-debug.apk`.
It is debug-signed and installs directly:

```bash
adb install -r packages/android/android/app/build/outputs/apk/debug/Markd-<version>-android-debug.apk
```

The debug build uses the application id `io.github.hafijbge.markd.debug`, so
it can be installed next to a release build.

For a release APK, run `pnpm --filter @markd/android build:apk:release`.
Without signing variables it produces an unsigned APK. To sign it, set
`MARKD_KEYSTORE`, `MARKD_KEYSTORE_PASSWORD`, `MARKD_KEY_ALIAS` and
`MARKD_KEY_PASSWORD`. CI reads the same values from the `MARKD_KEYSTORE_B64`
(base64 keystore), `MARKD_KEYSTORE_PASSWORD`, `MARKD_KEY_ALIAS` and
`MARKD_KEY_PASSWORD` secrets.

## Develop

```bash
pnpm --filter @markd/android dev
```

This runs the web app in a desktop browser at <http://localhost:5174>. Open and
save fall back to a file input and downloads there, and there is no stylus
handwriting. In the APK, debug builds allow remote debugging: open
`chrome://inspect` in desktop Chrome while the device is connected over adb.

## CI

- `.github/workflows/android.yml` builds a debug APK for pull requests that
  touch `packages/android` or `packages/muya`.
- `.github/workflows/release.yml` builds the APK on `v*` tags and attaches it
  to the GitHub release, next to the desktop builds. It signs the debug APK
  with the keystore in the `MARKD_DEBUG_KEYSTORE_B64` secret (base64; store and
  key password `android`, alias `androiddebugkey` unless
  `MARKD_DEBUG_KEYSTORE_PASSWORD` / `MARKD_DEBUG_KEY_ALIAS` /
  `MARKD_DEBUG_KEY_PASSWORD` say otherwise), so every release updates the
  previous install. Locally, set `MARKD_DEBUG_KEYSTORE` to the same file.
