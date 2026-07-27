# RPP product roadmap

This roadmap describes the direction of RPP after version 0.7.1.

It is not a promise that every item will ship in the listed order. Priorities
may change after real-world use exposes more important editing, stability, or
format problems.

## Product goal

RPP should become a practical 2D planning application for people who do not
want or need a professional CAD suite.

The finished product should remain:

- simple enough to use without a large manual;
- native and lightweight;
- fully usable offline;
- private by default;
- available as one portable Windows executable;
- precise enough for real dimensions and scaled printing;
- based on a readable, documented, versioned project language;
- English-first in source and file syntax, with a multilingual interface.

RPP is not intended to become a BIM platform, cloud collaboration service, or
Chromium-based application.

## Status vocabulary

| Status | Meaning |
|---|---|
| **Shipped** | Implemented in the current public build |
| **Partial** | A useful first implementation exists, but the complete workflow is missing |
| **Planned** | Accepted direction with no complete implementation yet |
| **Later** | Deliberately postponed until the core editor is mature |

## Current release: 0.7.1

### Shipped foundations

- Native Rust and egui Windows application.
- Portable single-file executable with no installer.
- Offline operation and no telemetry.
- Chained walls, diagonal walls, editable endpoints, thickness, type, color,
  locking, movement, duplication, deletion, and midpoint splitting.
- Configurable grid, snapping, coordinate display, exact Properties editing,
  gentle zoom, middle-button pan, and fit-to-plan.
- Temporary unsnapped measurement tool.
- Standalone colored lines.
- Wall-attached doors and windows with containment and overlap protection in
  normal editor workflows.
- Door types, hinge side, swing side, and window types.
- Free objects with built-in furniture categories, custom names, rotation,
  colors, locking, and eight direct resize handles.
- Text annotations with Arial regular/bold, size, rotation, and color.
- Manual polygon areas with fill, outline, and surface calculation.
- Compass roses and procedural vector arrows.
- Image importing, reusable project assets, content deduplication, optional
  processing limits, previews, removal, and one built-in symbol.
- Click selection, overlap cycling, `Shift+click`, `Ctrl+A`, grouped movement,
  nudging, duplication, recoloring, locking, unlocking, and deletion.
- 100-step undo/redo history.
- ZIP-compatible `.rpp` projects with readable RPP-PLAN data.
- Safe save through a temporary file, previous-file `.rpp.bak`, and local
  autosave recovery.
- SVG and PNG export.
- One-page PDF export at an exact scale on A4, Letter, Legal, or Tabloid.
- English (US), French, and Spanish interface.
- Light, Dark, and Warm dark flat themes.

### Partial foundations

- **Geometry editing:** basic wall splitting exists, but trim, extend, joins,
  parallels, alignment, and intersection repair are missing.
- **Rooms:** manual areas calculate surface, but walls do not yet generate
  rooms automatically.
- **Multiple selection:** grouped common operations exist, but selection
  rectangles, alignment, distribution, and grouping are missing.
- **Printing:** exact one-page PDF exists, but tiled multi-page printing and
  title blocks are missing.
- **Annotations:** text, lines, arrows, compass roses, and automatic wall
  lengths exist, but dedicated dimension objects, guides, axes, and legends
  are missing.
- **Image references:** imported images can be positioned and resized, but
  scale calibration from a known distance is missing.
- **Compatibility:** several early formats open, but a formal per-version
  migration framework and migration UI are still needed.

## Proposed milestones

## 0.8 — Geometry and rooms

The next major editing milestone should make RPP useful for plans that require
more than manually placed independent segments.

Planned work:

- endpoint, midpoint, intersection, and wall snapping;
- robust automatic wall joins;
- closed-contour repair within a documented tolerance;
- split at an arbitrary position;
- trim, extend, and shorten commands;
- parallel wall creation;
- alignment and distribution commands;
- crossing and overlap diagnostics;
- temporary guides and permanent reference axes;
- exact length and angle entry during creation;
- editable dimension labels;
- automatic room detection from closed walls;
- room name, type, surface, perimeter, fill, and visibility;
- open-contour diagnostics.

Acceptance emphasis:

- no invisible micro-gaps;
- no accidental plan-wide changes from one edit;
- deterministic room results;
- tests for diagonal, short, long, negative-coordinate, crossing, and nearly
  touching geometry.

## 0.9 — Organization and larger projects

Planned work:

- selection rectangle;
- copy, cut, and paste;
- object grouping and ungrouping;
- horizontal and vertical flipping;
- alignment and distribution for mixed selections;
- user-defined polygon objects;
- simple collision warnings;
- object favorites and reusable templates;
- layers with name, order, visibility, lock state, and opacity;
- object transfer between layers;
- multiple floors/levels;
- level duplication and consistent coordinates;
- lower-level underlay;
- ceiling height metadata;
- simple stair representation;
- minimap and zoom-to-selection;
- remembered panel layout and full-screen mode.

Performance work:

- spatial indexing for hit testing;
- cached geometry and room calculations;
- reduced redraw and recomputation;
- stress tests with several thousand items.

## 1.0 — Public-quality planning workflow

The 1.0 milestone should focus on completing workflows and making the
application suitable for a stable public release.

Planned work:

- JPEG export;
- configurable raster resolution and export region;
- transparent-background export;
- grid and annotation export controls;
- tiled multi-page PDF printing;
- margins and title blocks;
- image-reference calibration from a known distance;
- simple SVG import;
- CSV room and surface reports;
- free text, arrows, manual dimensions, automatic labels, legends, and project
  metadata managed as proper annotations;
- beginner-mode onboarding and an in-application tutorial;
- searchable shortcut reference;
- human-readable crash reports;
- optional previous-version retention policy;
- explicit migration preview and confirmation;
- read-only mode;
- complete file-format version documentation;
- signed release artifacts when distribution infrastructure exists;
- release packaging and signing improvements.

Release criteria:

- no known data-loss defect;
- parser, migrations, backups, recovery, undo, and redo covered by tests;
- useful performance with several thousand plan items;
- all core workflows available offline;
- English, French, and Spanish translation parity;
- current examples and documentation verified against the release executable.

## Post-1.0 — Advanced RPP-PLAN workflow

Planned after the graphical editor and file model are stable:

- object search by identifier;
- command console for instructions such as `WALL`, `DOOR`, and `ARROW`;
- live RPP-PLAN source preview;
- copy the current plan as RPP-PLAN text;
- paste an RPP-PLAN fragment into a project;
- bidirectional text/canvas synchronization;
- rectangular-room generator;
- walls-from-contour generator;
- standalone `.rpp` validator;
- automatic plan diagnostics and statistics;
- comparison between two project versions;
- reusable project templates.

This work must preserve comments and produce actionable line-level diagnostics
before live source editing can be considered safe.

## Later — Extensions and assisted generation

Deliberately postponed:

- extension or plugin API;
- third-party object libraries;
- AI-assisted plan creation through RPP-PLAN;
- optional 2D isometric visualization;
- accessibility expansion beyond the initial keyboard workflows.

No extension or AI feature should compromise offline use, project privacy, or
file-format stability.

## Feature-area matrix

| Area | Current state |
|---|---|
| Wall creation and basic editing | Shipped |
| Advanced wall operations | Planned |
| Grid, units, and basic precision | Shipped |
| Advanced snapping, guides, and dimensions | Planned |
| Doors and windows | Shipped |
| Automatic rooms | Planned |
| Manual colored areas and surfaces | Shipped |
| Basic furniture and objects | Shipped |
| Advanced grouping, collision, favorites | Planned |
| Click and additive selection | Shipped |
| Selection rectangle and clipboard | Planned |
| Zoom, pan, fit, coordinates | Shipped |
| Layers | Planned |
| Floors and levels | Planned |
| Text, lines, arrows, compass | Shipped |
| Dedicated dimensions, legends, metadata | Planned |
| RPP container and readable format | Shipped |
| Autosave, backup, legacy loading | Shipped |
| Formal migrations and read-only mode | Planned |
| SVG and PNG export | Shipped |
| One-page scaled PDF | Shipped |
| JPEG, CSV, tiled print, import calibration | Planned |
| English, French, Spanish | Shipped |
| Native offline Windows delivery | Shipped |
| Live source editor and console | Planned after 1.0 |
| Plugins and AI generation | Later |

## Architecture constraints

Future work must preserve these decisions unless a documented technical reason
justifies a change:

- Rust source code and English identifiers/comments/tests;
- egui native desktop interface;
- no Electron or bundled Chromium;
- no required network service;
- one portable Windows executable;
- centimeters as canonical internal geometry;
- English-only RPP-PLAN tokens;
- one `lang.yaml` containing every interface language;
- stable identifiers and validated cross-references;
- readable ZIP-compatible project container;
- bounded parser and image decoder;
- reversible edits through history snapshots or commands;
- deterministic export and save behavior.

## Definition of done

A persistent feature is not complete merely because a button is visible.

Depending on scope, completion requires:

1. usable interface behavior;
2. model representation;
3. undo and redo support;
4. RPP-PLAN parser and writer support;
5. validation and security limits;
6. SVG, PNG, and PDF behavior where relevant;
7. localization keys in all languages;
8. automated tests or a documented acceptance procedure;
9. public documentation and changelog entry;
10. compatibility consideration for older projects.
