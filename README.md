# RPP - Ran Plan Plan

RPP is a small native 2D floor-plan editor for Windows, macOS and Linux. I started it after failing to
find an open-source program that was easy to learn without being limited to
four lines and a prayer.

It sits somewhere between a sketching tool and a small CAD program. That is a
deliberate limit: RPP is for drawing a usable plan without first learning a
professional architecture suite.

The application is written in Rust with egui. It is native, works offline and
ships as a portable Windows executable, a macOS application, or a Linux binary.
There is no Electron bundle, account
or telemetry.

Current version: **0.9.1**. Expect missing tools and occasional rough edges
while the editor is still below 1.0.

## Download

Native builds are published on the
[Releases page](https://github.com/Bagyette/rpp-app/releases/latest):

| System | Download |
|---|---|
| Windows 10/11, x64 | `RPP-0.9.1-windows-x64.exe` |
| macOS, Apple Silicon | `RPP-0.9.1-macos-arm64.zip` |
| macOS, Intel | `RPP-0.9.1-macos-x64.zip` |
| Linux, x64 | `RPP-0.9.1-linux-x64.tar.gz` |

On Windows, run the executable directly. On macOS, extract `RPP.app` and place
it in Applications or your user Applications folder. On Linux, extract the
archive and run `./rpp`. The Linux binary is built on Ubuntu 24.04; it requires
compatible system libraries, including glibc 2.39 or later, OpenGL/EGL, X11 or
Wayland, and fontconfig. It is not a statically linked universal Linux package.
Native Linux file dialogs require a working desktop portal or `zenity`.

Windows builds are unsigned. macOS builds have an ad-hoc signature and are not
Apple notarized, so the operating system may require approval before opening.
`SHA256SUMS.txt` contains checksums for all four downloads.

Preferences and recovery files use each operating system's user directories:

| System | Preferences (`settings.json`, `language.txt`) | Recovery (`autosave.rpp`) |
|---|---|---|
| Windows | `%LOCALAPPDATA%\RPP\` | Same directory |
| macOS | `~/Library/Application Support/RPP/` | Same directory |
| Linux | `$XDG_CONFIG_HOME/rpp/` (default `~/.config/rpp/`) | `$XDG_DATA_HOME/rpp/` (default `~/.local/share/rpp/`) |

Existing Windows preferences remain in their original location. Recovery keeps
unsaved changes and the original project path; clean or discarded sessions are
not restored. Undo/redo updates the modified flag by comparing with the last
saved content. Project files remain portable between systems.

File opening, save/recovery compression and SVG/PNG/PDF export run in a serialized
background queue. Automatic room detection also runs there above 128 walls.
Foreground operations show an activity indicator and temporarily pause editing;
recovery runs while editing continues. Snapshot cloning, validation, painting and
small room calculations still use the UI thread, so this is not a guarantee of
zero pauses on maximum-size plans.

## The first five minutes

Pick a tool on the left, draw on the canvas, then edit the selected item from
the Properties panel. The mouse wheel zooms and the middle mouse button moves
the view. Trackpad scrolling pans; pinching zooms around the pointer when the
windowing backend supplies pinch events. Space + left-drag also pans, so a
mouse with no middle button works. Shift + wheel pans horizontally, and
Alt/Option + scrolling zooms. In Settings, scrolling can be forced to pan or
zoom for devices that report ambiguous scroll events, such as some touch mice.
Linux/Windows drivers that do not expose native pinch events can use modified
scrolling, the keyboard shortcuts or the on-screen zoom controls.

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
- multi-selection, rectangular selection, alignment and up to 100 undo/redo states
  within a 64 MiB history allocation budget;
- millimeters, centimeters, meters, inches and feet;
- SVG, PNG and vector one-page PDF export at a chosen scale;
- English, French and Spanish;
- Light, Dark and Warm Dark themes.

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
| New / open | `Ctrl+N` / `Ctrl+O` |
| Save | `Ctrl+S` |
| Save as | `Ctrl+Shift+S` |
| Select all | `Ctrl+A` |
| Duplicate | `Ctrl+D` |
| Undo / redo | `Ctrl+Z` / `Ctrl+Y` |
| Zoom in / out | `Ctrl++` / `Ctrl+-` |
| Fit the plan | `Home` or `Ctrl+0` |
| Cancel | `Escape` |

Use `Command` (`⌘`) instead of `Ctrl` on macOS. Letter shortcuts follow the
logical letter printed on the active keyboard layout (including AZERTY and
QWERTY), not a fixed US key position. Text fields keep their own editing
shortcuts; tool keys do not fire during typing or with command/Alt modifiers.

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

Install stable Rust (1.92 or later). Windows requires the MSVC C++ build tools;
macOS requires the Xcode Command Line Tools. On Debian/Ubuntu, install:

```sh
sudo apt-get install pkg-config libxkbcommon-dev libwayland-dev libx11-xcb-dev \
  libegl1-mesa-dev libgl1-mesa-dev libdbus-1-dev libfontconfig1-dev \
  fonts-liberation fonts-dejavu-core
```

Then, on any supported operating system:

```sh
cd native
cargo fmt --all --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --release --locked
```

The executable is written to `native/target/release/rpp` (`rpp.exe` on Windows).
`rpp path/to/project.rpp` opens a project; dropping a file into the window also
opens it, with a confirmation if the current plan has unsaved edits.

To package a macOS app locally, run from the repository root:

```sh
python3 scripts/package_release.py --platform macos --architecture arm64
```

Use `x64` for an Intel Mac. The same script accepts `windows` and `linux`.
GitHub Actions runs formatting, Clippy, tests and release builds on all four
platform/architecture combinations. On `main`, a new version in `Cargo.toml`
is published only after all jobs pass; an existing release is left unchanged.
See [STYLE.md](STYLE.md) for code conventions and the Google Rust style review.
The [0.9.1 audit report](AUDIT.md) records fixes, verification and remaining limits.

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
    ├── input.rs             logical shortcuts and canvas navigation
├── platform.rs          user directories and system fonts
├── main.rs              application entry point
    └── model.rs             saved project data
```

The source, identifiers, comments and RPP-PLAN keywords are written in English.
Interface text belongs in `native/lang.yaml`; all languages must contain the
same keys.

## Tests and bug reports

`cargo test` covers the editor, input handling, project format and exports. They cover project round trips, malformed
files, old formats, embedded assets, unit conversion, image processing,
selection behavior and each export format. Release builds are also opened and
checked manually because unit tests do not catch canvas and menu bugs.

For a useful bug report, include the RPP and operating-system versions, the steps that
trigger the problem, and what should have happened. A screenshot or a small
`.rpp` file helps a lot for geometry and rendering problems.

## License

RPP uses the [BSD Zero Clause License](LICENSE) (`0BSD`). You may use, modify
and redistribute it without an attribution requirement. The software is
provided `"AS IS"`, without warranty.
