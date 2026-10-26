# Inkwell

Markd's home screen and its drawing / PDF-annotation canvas, modelled on
Xournal++ 1.3.8. It is an Android library module (`com.hafij.inkwell`) built
into the Markd app; see [the Android README](../../README.md) for what users see.

- `HomeActivity` is the launcher. It lists one SAF folder the user picks
  (Markdown, `.ink` drawings and annotations, PDFs, subfolders) with a Recent
  row and a **+** menu. Markdown opens in Markd's `MainActivity`.
- `EditorActivity` edits a note. Notes in the folder are copied to
  `cache/share/work/` while open and written back after every save.
  `SketchActivity` is the same canvas for Markd's Insert → Sketch; it returns an
  SVG with the note embedded in `<metadata><inkwell:note>` (`io/SvgSketch.java`).
- Notes are `.ink` files in the Xournal++ file structure (gzip XML,
  fileversion 4); `.xopp` files read the same way. A PDF annotation links to its
  PDF (`domain="absolute"` file name plus an `inkwell-uri` content URI) and
  never changes it. Notes from 0.24.0 with an attached `<note>.ink.bg.pdf` still
  open.
- Export writes a vector PDF with PdfBox-Android: the original pages are copied
  as they are, and ink, text and images are drawn into the page content.
- Tools: pen (5 widths, solid/dash/dash-dot/dot, fill, pressure, opacity),
  highlighter (opacity, 50% by default), eraser (touched part or whole stroke),
  text, image, line/rectangle/ellipse/arrow/double arrow/axes, shape
  recogniser, lasso and rectangle select (move, resize, duplicate,
  copy/cut/paste, recolour), hand tool, undo/redo, pages, layers, stylus-only
  mode.

It builds with the app (`pnpm --filter @markd/android build:apk`).

Icons are Lucide (ISC), as shipped with Xournal++; `tools/svg2vector.py`
converts the SVGs to VectorDrawables. PDF engine: PdfBox-Android 2.0.27.0
(Apache 2.0).
