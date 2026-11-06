RPP 0.9.2 improves recovery, undo/redo, PDF quality and responsiveness on all four native builds.

- Recovery retains the original project path and unsaved state. Saved, discarded,
  new and undo-to-saved sessions no longer reopen as unsaved documents.
- Undo/redo updates the modified flag by comparing with the last saved content.
- PDF export keeps walls, shapes and text as vector geometry. Text is outlined,
  so it stays sharp when zooming but cannot be selected/copied as text. Imported
  images remain raster. The obsolete PDF font dependencies have been removed.
- Undo and redo share an estimated 64 MiB allocation budget and a 100-state limit;
  shared image resources are counted once.
- Opening, save/recovery compression and exports run in a serialized background
  queue. Room detection above 128 walls runs there too, with an activity indicator.
  Foreground tasks temporarily pause editing; recovery continues while editing.
- Updated RPP_FORMAT.md, including the internal recovery envelope. Portable
  project files still use the ZIP container and RPP-PLAN 2 language.

Validation: 87 regression tests, strict Clippy, formatting, dependency audit and
native builds are checked by GitHub Actions for Windows x64, Linux x64,
macOS Apple Silicon and macOS Intel before publication.

Snapshot cloning, validation and painting still use the UI thread; very large
plans can still pause briefly. Windows is portable and unsigned. macOS apps
have an ad-hoc signature and are not Apple notarized. Linux requires an
Ubuntu 24.04-compatible runtime. See AUDIT.md for practical limits.
