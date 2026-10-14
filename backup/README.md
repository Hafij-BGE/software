# backup/

Copies of what the builds download from ArtCraft's GitHub (github.com/storytold), so Craft Suite
still builds if those repositories ever disappear:

- `craft-libs.bundle`: the craft-libs repository as one git bundle (branches, tags, full history,
  and every commit the `Cargo.lock` files pin). To get the code back out:
  `git clone craft-libs.bundle craft-libs`.
- `craft-fonts/<commit>/fonts/`: the fonts each app's release embeds, at the commit its
  `.github/workflows/release.yml` pins (`craft-fonts-ref`).

The builds use these on their own when github.com/storytold can't be reached (`packaging/lib.sh`:
`craft_libs_fallback`, `fonts_dir`). `scripts/backup-deps.sh` refreshes them; the daily upstream
sync runs it after each update. The fonts keep their own licences (`OFL.txt` next to each).
