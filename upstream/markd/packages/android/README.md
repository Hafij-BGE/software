# Markd for Android

Android app for Markd. A small native shell (Java, one `WebView`) hosts the
standalone `@muyajs/core` editor from `packages/muya`. The Electron renderer in
`packages/desktop` is not used, because it depends on Electron IPC.

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
- Stylus:
  - **Pen handwriting to text** (Android 14+). Write on the page with the pen
    and Android turns the writing into text at that spot. The editor never
    cancels pen events. Draw → *Pen to text* turns this on or off
    (`View.setAutoHandwritingEnabled`).
  - **Sketch pad** (Insert/Draw → *Sketch*). It has pressure-sensitive ink, a
    highlighter, and a stroke eraser. Tilting the pen gives broader strokes, and
    hovering shows a preview of the tip.
  - **Palm rejection**: touches are ignored while a pen is in use.
  - The pen's eraser end, or its side button, erases strokes.
  - Undo and redo work in the sketch pad.
  - The sketch is inserted as a PNG or SVG in `assets/` next to the `.md` file.
    If the app cannot reach that folder, the image is embedded in the document.
  - The status bar shows pen hover, pressure, and tilt.

## How it fits together

```
src/                 web app (TypeScript, Vite) → android/app/src/main/assets/www
  editor.ts          muya setup; maps relative image paths to the /doc/ route
  ribbon.ts          ribbon tabs and groups
  document.ts        open / save / recent files / image assets (via native.ts)
  ink.ts, pen.ts     sketch pad and stylus status
  export.ts          HTML / PDF / DOCX
android/             Gradle project
  MainActivity.java  WebView host, serves https://appassets.androidplatform.net
  Bridge.java        JS ⇄ native calls (SAF pickers, file IO, printing)
  DocStore.java      Storage Access Framework helpers
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
  to the GitHub release, next to the desktop builds.
