# Third-party notices

Craft Suite (Copyright (c) 2026 Md. Hafijur Rahman, MIT OR Apache-2.0, see `LICENSE-MIT` and
`LICENSE-APACHE`) bundles the following projects. Each keeps its own licence and copyright; the
full texts and the per-file attribution lists are in each project's folder under `upstream/` and
are installed next to the app (Linux: `/usr/share/doc/craft-suite/`, Windows: the `licences`
folder).

| Project | Used as | Copyright | Licence | Source |
| --- | --- | --- | --- | --- |
| WordCraft | Word tab | Copyright (c) 2026 ArtCraft Team and the WordCraft contributors | MIT OR Apache-2.0 | https://github.com/storytold/wordcraft |
| DeckCraft | Slide tab | Copyright (c) 2026 ArtCraft Team and the DeckCraft contributors | MIT OR Apache-2.0 | https://github.com/storytold/deckcraft |
| GridCraft | Spreadsheet tab | Copyright (c) 2026 ArtCraft Team and the GridCraft contributors | MIT OR Apache-2.0 | https://github.com/storytold/gridcraft |
| Markd | Markd tab | Copyright (c) 2017-present Luo Ran; Copyright (c) 2018-present MarkText Contributors; Markd changes by Md. Hafijur Rahman | MIT | https://github.com/Hafij-BGE/markd |
| MarkText (basis of Markd) | via Markd | Luo Ran and MarkText Contributors | MIT | https://github.com/marktext/marktext |

The ArtCraft name, wordmark and logos are trademarks of the ArtCraft Team and are not covered by
those licences. Craft Suite does not use them as its own branding.

## Further material shipped with the apps

- Fonts: Inter, Carlito (metric-compatible with Calibri), Caladea (metric-compatible with
  Cambria), Liberation, Lato, Open Sans, Roboto, Merriweather, Playfair Display, Poppins, Nunito
  Sans, Montserrat, BIZ UD, Shippori Mincho, Noto Sans Arabic, Source Sans 3, Source Serif 4,
  JetBrains Mono, Ubuntu Light, Hack, Noto Emoji and DejaVu Sans Mono, under the SIL Open Font
  Licence 1.1 or the font's own free licence (Bitstream Vera, Ubuntu Font Licence). The
  per-file list is in `upstream/*/ATTRIBUTION.md` and the licence texts ship with the packages.
  No Microsoft font files are bundled; document font names such as Calibri or Aptos are only
  referenced so that documents keep their layout.
- Lucide icons (ISC; Feather-derived icons MIT), embedded in the apps' Insert > Icons gallery.
  See `upstream/*/NOTICE`.
- Templates: Craft Suite's own are CC0; the open-licence ones are credited with author and
  licence in `templates/ATTRIBUTION.md`.
- Rust crates (egui, eframe, wry and others) under MIT, Apache-2.0 or similar licences; see
  `suite/Cargo.lock` and each crate's own licence.
- Markd's editor libraries (Muya, DejaVu Sans Mono, Open Sans, Noto Music and Noto Sans Math
  fonts and others) under their own licences; see `upstream/markd/LICENSE` and its package
  metadata.

## Not affiliated

Craft Suite is an independent project. It is not affiliated with, or endorsed by, Microsoft or
any other vendor whose file formats it reads and writes.
