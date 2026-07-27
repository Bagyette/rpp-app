# RPP — Ran Plan Plan

RPP is a lightweight, native 2D floor-plan editor for Windows.

It is designed for people who want to sketch and annotate a plan without
learning a full architectural CAD suite. RPP starts quickly, works completely
offline, stores projects in a documented readable format, and ships as one
portable executable.

> RPP is under active development. Version 0.7.1 is already useful for simple
> plans, furniture layouts, annotations, image-based symbols, measurements, and
> scaled exports, but it is not yet a replacement for professional CAD or BIM
> software.

## Highlights

- Native Rust application: no Electron, bundled Chromium, local web server, or
  separate runtime.
- One portable Windows `.exe`; no installer.
- Offline by design. RPP does not send project data anywhere.
- English (US), French, and Spanish interface.
- Readable and documented RPP-PLAN project language.
- Reusable images embedded directly in the project.
- SVG, PNG, and one-page scaled PDF export.
- Automatic recovery file, previous-file backup, and 100-step undo history.
- Light, Dark, and Warm dark flat themes.

## Download and run

Download the current Windows build from
[GitHub Releases](https://github.com/Bagyette/rpp-app/releases/latest).

The current release executable is:

```text
RPP-Native-0.7.1.exe
```

Double-click it to start. No installation, Internet connection, browser, or
administrator privileges are required.

Windows may display a SmartScreen warning for an unsigned development build.
Check that the file came from the expected release before running it.

RPP stores local preferences and recovery data in:

```text
%LOCALAPPDATA%\RPP\
├── autosave.rpp
├── language.txt
└── settings.json
```

## What RPP can do

### Drawing and precision

- Create chained horizontal, vertical, or diagonal walls.
- Edit wall endpoints, thickness, interior/exterior type, color, and lock
  state.
- Move, duplicate, delete, and split walls at their midpoint.
- Draw independent straight lines with their own color and lock state.
- Display wall and line lengths.
- Use a configurable grid and optional snapping.
- Enter exact coordinates and dimensions from the Properties panel.
- Work in millimeters, centimeters, meters, inches, or feet/inches.
- Keep centimeters as the canonical internal unit, preventing unit changes
  from altering the actual geometry.
- Measure freely between two unsnapped points. A temporary measurement
  disappears when the mouse button is released.

### Doors and windows

- Attach doors and windows to walls.
- Move openings along their wall while preserving the attachment.
- Edit their exact width and normalized position.
- Use single, double, or sliding doors.
- Configure door hinge side and inward/outward swing.
- Use standard or bay windows.
- Prevent openings from extending beyond a wall or overlapping another
  opening.
- Remove an opening automatically if its wall becomes too short to contain it.

### Objects, text, and annotations

- Place rectangle, bed, table, sofa, wardrobe, and desk objects.
- Give an object a custom display name without changing its underlying type.
- Edit position, width, height, rotation, fill, outline, and lock state.
- Resize selected objects using eight rotation-aware handles.
- Hold `Shift` while resizing a corner to preserve the aspect ratio.
- Add free text with position, size, rotation, color, and regular/bold Arial
  styling.
- Add movable and customizable compass roses.
- Add vector arrows with editable length, width, color, lock state, four
  styles, and exactly 64 orientation steps of 5.625 degrees.
- Create colored polygon areas, including concave shapes, and display their
  calculated surface.

### Images and built-in symbols

- Import PNG, JPEG, GIF, BMP, and WebP files.
- Embed assets in the `.rpp` project so the plan stays portable.
- Place the same asset multiple times without embedding duplicate bytes.
- Detect identical imported content.
- Preview each asset with its aspect ratio, pixel dimensions, embedded size,
  format, and default placement dimensions.
- Limit imported images to 8, 16, 24, 32, 48, 64, 96, 128, 192, or 256 KiB.
- Select `Source` to keep the original bytes unchanged.
- Preserve transparency with PNG and encode opaque images as JPEG when
  processing is enabled.
- Keep new image instances at no more than 40 cm on their longest side by
  default.
- Remove an imported asset and all its instances as one undoable action.
- Reuse the built-in emergency-exit symbol without adding its bytes to every
  project.

### Selection and editing

- Select an item by clicking it.
- Click repeatedly to cycle through overlapping items.
- Use `Shift+click` to add or remove individual items from a selection.
- Use `Ctrl+A` to select every unlocked item.
- Move, keyboard-nudge, duplicate, recolor, lock, unlock, or delete multiple
  compatible items together.
- Move walls, lines, objects, images, text, areas, compass roses, and arrows.
- Move doors and windows along their receiving wall.
- Undo or redo up to 100 committed edits.

### Navigation, saving, and export

- Zoom gently with the mouse wheel.
- Pan with the middle mouse button.
- Fit the complete plan into the current view.
- Display cursor coordinates, active unit, active tool, item counts, and zoom.
- Save a project as `.rpp`.
- Create a `.rpp.bak` copy before replacing an existing project.
- Maintain `%LOCALAPPDATA%\RPP\autosave.rpp` after committed changes.
- Export the complete plan as vector SVG.
- Export as PNG.
- Include or hide wall and line distances in SVG and PNG exports.
- Export a one-page PDF at an exact `1:N` drawing scale.
- Choose A4, US Letter, US Legal, or US Tabloid paper.
- Automatically select portrait or landscape orientation.
- Refuse a PDF export instead of silently clipping a plan that does not fit.

## Tools and shortcuts

| Action | Shortcut | Behavior |
|---|---:|---|
| Select | `V` | Select, move, resize, and edit plan items |
| Wall | `M` | Click successive wall vertices; `Shift` constrains an active segment |
| Line | `L` | Drag a standalone straight line |
| Measure | `R` | Drag between two free, unsnapped points |
| Text | `T` | Click to place a text annotation |
| Door | `P` | Click near a wall to place a door |
| Window | `F` | Click near a wall to place a window |
| Object | `O` | Click to place a rectangular object |
| Area | `A` | Click polygon points; use `Enter`, double-click, or right-click to close |
| Compass | `N` | Click to place a compass rose |
| Arrow | `Y` | Drag from the tail to the tip |
| Save | `Ctrl+S` | Save to the current file or ask for a destination |
| Select all | `Ctrl+A` | Select all unlocked plan items |
| Duplicate | `Ctrl+D` | Duplicate the complete current selection |
| Undo | `Ctrl+Z` | Undo the previous committed edit |
| Redo | `Ctrl+Y` or `Ctrl+Shift+Z` | Restore an undone edit |
| Delete | `Delete` or `Backspace` | Delete the current selection |
| Nudge | Arrow keys | Move by 1 cm internally |
| Large nudge | `Shift` + Arrow keys | Move by 10 cm internally |
| Fit plan | `Home` | Fit all plan content in the viewport |
| Cancel | `Escape` | Cancel the active operation and clear the selection |
| Pan | Middle-button drag | Move the viewport |
| Zoom | Mouse wheel | Zoom around the pointer |

Shortcuts are ignored while editing text where necessary.

## The `.rpp` project format

An `.rpp` file is a ZIP-compatible container:

```text
my-project.rpp
├── plan.data
└── assets/
    ├── asset-123.png
    └── asset-456.jpg
```

`plan.data` is UTF-8 RPP-PLAN text. Its keywords are always English, regardless
of the interface language, so the same project behaves consistently on every
computer.

Rename a copy of an `.rpp` file to `.zip` to inspect it with a normal archive
tool. See [FORMAT_RPP.md](FORMAT_RPP.md) for the complete grammar, validation
rules, examples, compatibility notes, and security limits.

## Current limitations

RPP 0.7.1 deliberately remains smaller than a professional architectural
package. It currently does **not** provide:

- automatic room detection from walls;
- wall join, trim, extend, parallel, or alignment commands;
- selection rectangles, clipboard copy/paste, or object grouping;
- layers, floors, ceiling heights, or stairs;
- automatic collision detection;
- guides, reference axes, manual dimension objects, title blocks, or legends;
- DXF, DWG, or SVG import;
- JPEG export, CSV room reports, or multi-page/tiled printing;
- live RPP-PLAN text editing or bidirectional text/canvas synchronization;
- 3D or isometric projection;
- plugins or AI-generated plans.

These items belong to the roadmap and should not be interpreted as completed
features. See [ROADMAP_RPP_FINAL.md](ROADMAP_RPP_FINAL.md).

## Build from source

### Requirements

- Windows 10 or Windows 11;
- a current stable Rust toolchain with Cargo;
- the Windows development components required by Rust's selected MSVC target.

### Build

```powershell
cd native
cargo test
cargo build --release
```

The optimized executable is produced at:

```text
native\target\release\rpp.exe
```

The release profile optimizes for binary size, enables fat LTO, uses one code
generation unit, strips symbols, and aborts on panic.

## Repository layout

```text
.
├── native/
│   ├── assets/                 Built-in icons and symbols
│   ├── lang.yaml               All interface translations
│   ├── src/
│   │   ├── app.rs              Application state and interactions
│   │   ├── app/inspectors.rs   Properties and library panels
│   │   ├── app/render.rs       Canvas rendering
│   │   ├── export.rs           SVG, PNG, and PDF output
│   │   ├── format.rs           RPP container and RPP-PLAN parser
│   │   ├── localization.rs     Language catalog loader
│   │   ├── main.rs             Native application entry point
│   │   └── model.rs            Persistent project model
│   └── Cargo.toml
├── CHANGELOG.md
├── FORMAT_RPP.md
├── LICENSE
├── LOCALIZATION.md
├── ROADMAP_RPP_FINAL.md
├── example-room.data
└── example-room.rpp
```

## Testing

Run:

```powershell
cd native
cargo test
```

The current suite covers 36 cases, including:

- readable RPP-PLAN round trips;
- packaged projects and embedded assets;
- legacy JSON and `RPPBIN` files;
- invalid syntax, references, geometry, and oversized inputs;
- non-metric unit conversion;
- built-in assets remaining outside the project archive;
- image processing limits and deduplication behavior;
- selection behavior and gentle zoom;
- SVG, PNG, and scaled PDF generation;
- interface translation key parity.

Release builds are additionally smoke-launched to detect immediate startup
failures. Automated tests do not replace visual and interaction testing; UI
problems should be reported with reproduction steps and, when useful, a
screenshot and sample `.rpp` file.

## Contributing and issue reports

When reporting a problem, include:

1. the RPP version;
2. the Windows version;
3. exact reproduction steps;
4. what happened and what you expected;
5. a screenshot or minimal `.rpp` file when relevant.

Before proposing a code change, keep these project rules in mind:

- source code, identifiers, comments, tests, and RPP-PLAN keywords are English;
- user-facing strings belong in `native/lang.yaml`;
- all languages must expose the same translation keys;
- the application must remain native, offline, simple, and portable;
- new persistent data requires parser, writer, validation, export, undo/redo,
  documentation, and compatibility consideration.

## Documentation

- [RPP project and RPP-PLAN specification](FORMAT_RPP.md)
- [Localization guide](LOCALIZATION.md)
- [Product roadmap](ROADMAP_RPP_FINAL.md)
- [Changelog](CHANGELOG.md)

## License

RPP is released under the permissive
[BSD Zero Clause License](LICENSE), identified by SPDX as `0BSD`.

You may use, copy, modify, and distribute the software for any purpose, with or
without fee, and without an attribution condition. The software is provided
`"AS IS"` without warranty; read the short license text for the complete terms.
