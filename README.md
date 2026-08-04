# RPP (Ran Plan Plan)

RPP is a small 2D floor-plan editor for Windows. I started it after failing to
find an open-source program that was easy to learn without being limited to
four lines and a prayer.

It sits somewhere between a sketching tool and a small CAD program. That is a
deliberate limit: RPP is for drawing a usable plan without first learning a
professional architecture suite.

The application is written in Rust with egui. It is native, works offline and
ships as one portable `.exe`. There is no Electron bundle, installer, account
or telemetry.

Current version: **0.8.0**. Expect missing tools and occasional rough edges
while the editor is still below 1.0.

## Download

Windows builds are published on the
[Releases page](https://github.com/Bagyette/rpp-app/releases/latest).

Download `RPP-Native-0.8.0.exe` and run it directly. Windows SmartScreen may
warn about the file because development builds are not signed.

Preferences and recovery files are kept in `%LOCALAPPDATA%\RPP\`:

```text
autosave.rpp
language.txt
settings.json
```

## The first five minutes

Pick a tool on the left, draw on the canvas, then edit the selected item from
the Properties panel. The mouse wheel zooms and the middle mouse button moves
the view.

The wall tool accepts successive points, so a room outline can be drawn without
restarting the tool after every segment. Doors and windows are placed directly
on a wall. Objects, text, images, areas, room boundaries, arrows and compass
roses are independent items.

Measurements, area points and manual room boundaries are free by default. Hold
`Shift` while placing a point when it needs to land on the grid. A temporary
measurement disappears as soon as the mouse button is released.

Walls always have a small screen-space magnet around existing wall endpoints
and segments. This closes contours cleanly even when normal grid snapping is
disabled.

What already works:

- horizontal, vertical and diagonal walls, with editable endpoints and
  thickness;
- doors, windows, standalone lines, furniture objects and custom labels;
- text using Arial regular or bold, with color, size and rotation;
- polygon areas with fill color and calculated surface;
- automatic rooms from closed walls, plus manual room boundaries for open or
  deliberately divided spaces;
- room names, types, surface, perimeter and optional fill;
- ordered layers with visibility, locking, opacity and per-item assignment;
- reusable PNG, JPEG, GIF, BMP and WebP assets stored inside the project;
- multi-selection, rectangular selection, alignment and 100 undo steps;
- millimeters, centimeters, meters, inches and feet;
- SVG, PNG and one-page PDF export at a chosen scale;
- English, French and Spanish;
- Light, Dark and Warm Dark themes.

The detailed feature list and planned work live in
[ROADMAP_RPP_FINAL.md](ROADMAP_RPP_FINAL.md). That file distinguishes what is
implemented from what is only planned.

## Main shortcuts

| Action | Shortcut |
|---|---:|
| Select | `V` |
| Wall | `M` |
| Line | `L` |
| Measure | `R` |
| Text | `T` |
| Door | `P` |
| Window | `F` |
| Object | `O` |
| Area | `A` |
| Manual room boundary | `B` |
| Compass rose | `N` |
| Arrow | `Y` |
| Save | `Ctrl+S` |
| Save as | `Ctrl+Shift+S` |
| Select all | `Ctrl+A` |
| Duplicate | `Ctrl+D` |
| Undo / redo | `Ctrl+Z` / `Ctrl+Y` |
| Fit the plan | `Home` |
| Cancel | `Escape` |

Arrow keys move the current selection by 1 cm. Add `Shift` for a 10 cm step.
`Delete` and `Backspace` remove the selection.

## Project files

An `.rpp` project is a ZIP-compatible container:

```text
my-plan.rpp
├── plan.data
└── assets/
    ├── asset-123.png
    └── asset-456.jpg
```

`plan.data` contains readable UTF-8 text written in the RPP-PLAN language. Its
keywords stay in English even when the interface uses another language. This
keeps project files portable and makes it possible to inspect or write a small
plan by hand.

Renaming a copy from `.rpp` to `.zip` is enough to open the container with a
normal archive tool. The syntax, validation rules and compatibility notes are
documented in [RPP_FORMAT.md](RPP_FORMAT.md).

## Rough edges

RPP is useful, but it is not finished. There are no multiple floors, wall
trim/extend tools, clipboard, tiled printing, DXF/DWG import or 3D view yet.
Imported SVG files are not supported either. If a workflow depends on one of
those, the current build is simply not the right tool for it yet.

File compatibility is still pre-1.0. Old project variants are accepted where a
migration already exists, but the format may gain fields as the editor grows.
Keep backups of important plans.

## Build from source

The build currently targets Windows 10 and 11. Install a stable Rust toolchain
with the MSVC target, then run:

```powershell
cd native
cargo test
cargo build --release
```

The executable is written to `native\target\release\rpp.exe`.

The release profile favors a small binary: size optimization, fat LTO, one code
generation unit and stripped symbols are enabled in `Cargo.toml`.

## Repository layout

```text
native/
├── assets/                  built-in icons and symbols
├── lang.yaml                every interface translation
└── src/
    ├── app.rs               state and editor interactions
    ├── app/inspectors.rs    properties and image library
    ├── app/render.rs        canvas rendering
    ├── export.rs            SVG, PNG and PDF export
    ├── format.rs            .rpp reader and writer
    ├── localization.rs      language catalog
    ├── main.rs              application entry point
    └── model.rs             saved project data
```

The source, identifiers, comments and RPP-PLAN keywords are written in English.
Interface text belongs in `native/lang.yaml`; all languages must contain the
same keys.

## Tests and bug reports

`cargo test` currently runs 47 tests. They cover project round trips, malformed
files, old formats, embedded assets, unit conversion, image processing,
selection behavior and each export format. Release builds are also opened and
checked manually because unit tests do not catch canvas and menu bugs.

For a useful bug report, include the RPP and Windows versions, the steps that
trigger the problem, and what should have happened. A screenshot or a small
`.rpp` file helps a lot for geometry and rendering problems.

## Documentation

- [RPP-PLAN and the `.rpp` container](RPP_FORMAT.md)
- [Localization](LOCALIZATION.md)
- [Roadmap](ROADMAP_RPP_FINAL.md)
- [Changes by version](CHANGELOG.md)

## License

RPP uses the [BSD Zero Clause License](LICENSE) (`0BSD`). You may use, modify
and redistribute it without an attribution requirement. The software is
provided `"AS IS"`, without warranty.
