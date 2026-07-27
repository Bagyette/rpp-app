# Changelog

All notable RPP changes are documented here.

RPP is still pre-1.0. Minor releases may add project fields, but the reader is
expected to keep opening documented older formats and to convert them
explicitly when saved.

## Unreleased

### Added

- Added the BSD Zero Clause License (`0BSD`).
- Declared `0BSD` in the Rust package metadata.

## 0.7.1

### Fixed

- Fixed Solid arrows rendering as malformed, self-crossing shapes in the
  editor.
- Rendered Solid arrows as two valid convex pieces: shaft and arrowhead.
- Prevented clicks in menus and the Settings window from passing through to
  the plan canvas.

### Documentation

- Reworked the public README, RPP-PLAN specification, localization guide,
  roadmap, and changelog for a public repository.

## 0.7.0

### Added

- Added procedural vector arrows.
- Added `open`, `open-bold`, `hollow`, and `solid` arrow styles.
- Added exactly 64 arrow orientation steps of 5.625 degrees.
- Added a minimalist circular orientation control.
- Added custom object display names.
- Added eight rotation-aware direct resize handles to objects.
- Added aspect-ratio preservation with `Shift` during corner resize.
- Expanded multiple selection with grouped movement, keyboard nudging,
  duplication, recoloring, locking, unlocking, and deletion.
- Added inches and feet/inches display units while keeping centimeters
  canonical internally.
- Added A4, Letter, Legal, and Tabloid paper choices for one-page scaled PDF
  output.
- Added a dedicated Settings window.
- Added Light, Dark, and Warm dark flat themes.
- Added complete Spanish interface strings beside English and French.
- Added a configurable imported-image target from 8 to 256 KiB plus `Source`.
- Added alpha-aware PNG processing and JPEG processing for opaque images.
- Added richer image-library previews and embedded-size information.
- Added content-based imported-image deduplication.
- Added the built-in emergency-exit symbol without embedding it in projects.
- Added RPP-PLAN `OBJECT_NAME` and `ARROW` instructions.
- Added `mm`, `cm`, `m`, `in`, and `ft` input-unit declarations.

### Changed

- Enlarged visible undo and redo arrows by cropping transparent padding at
  runtime.
- Used ZIP Deflate for BMP assets and stored already-compressed image formats
  directly.
- Extended SVG, PNG, PDF, project validation, undo/redo, selection, and bounds
  calculation for the new item types.
- Updated the Properties panel for arrow and object editing.

### Tests

- Added non-metric unit conversion coverage.
- Added built-in asset container coverage.
- Added image processing limit coverage.
- Added complete translation-key parity checks.
- Added current RPP-PLAN round-trip coverage for object names and arrows.

## 0.6.2

### Added

- Added `Ctrl+A` selection for every unlocked plan item.
- Added embedded undo and redo image icons.

### Changed

- Split large rendering and Properties-panel sections out of `app.rs`.
- Enlarged undo and redo controls for better visibility.

## 0.6.1

### Added

- Added `Shift+click` additive selection.
- Added grouped deletion for multiple selected items.

### Fixed

- Removed grid snapping from the temporary Measure tool.

## 0.6.0

### Added

- Added movable compass roses with size, rotation, color, and locking.
- Added manual polygon areas and surface calculation.
- Added millimeter, centimeter, and meter display units.
- Added remembered settings and default drawing values.
- Added configurable colors across walls, lines, text, openings, objects,
  areas, and compass roses.
- Added regular and bold Arial text support using Windows fonts.
- Added one-page A4 PDF export at an exact drawing scale.

## 0.5.9

### Added

- Added imported-asset removal with undo support.
- Added repeated-click cycling through overlapping plan items.

### Changed

- Limited default imported-image placement to 40 cm on its longest side.

## 0.5.8

### Changed

- Reduced binary size by sharing the Deflate implementation and trimming
  renderer features.
- Kept release optimization focused on one small portable executable.

## 0.5.7

### Added

- Added complete-plan SVG export.
- Added complete-plan PNG export.
- Added export controls under Save as.

## 0.5.6

### Added

- Added fit-to-plan navigation.
- Added keyboard nudging for selected items.

## 0.5.5

### Fixed

- Reduced mouse-wheel zoom sensitivity.
- Bounded zoom between the supported minimum and maximum.

## 0.5.4

### Fixed

- Fixed door type, hinge, and swing controls that appeared editable but did
  not update reliably.
- Fixed equivalent Properties controls for windows and objects.

## 0.5.3

### Added

- Added standard and bay windows attached to walls.
- Added rectangle, bed, table, sofa, wardrobe, and desk objects.
- Added opening overlap checks in normal editor workflows.

## 0.5.2

### Added

- Replaced the early standalone plan file with a ZIP-compatible `.rpp`
  container.
- Added readable `plan.data`.
- Added embedded project assets under `assets/`.
- Added reusable image instances referencing one embedded asset.
- Added image movement, exact resizing, rotation, duplication, and locking.

## Earlier prototypes

The initial prototypes established:

- a native Rust and egui application;
- chained wall creation;
- wall properties and wall-attached doors;
- grid snapping and zoom/pan;
- basic save, open, undo, redo, and recovery;
- the first readable RPP-PLAN syntax;
- English and French interface localization.

Those builds were experimental and predate the current documented project
container.
