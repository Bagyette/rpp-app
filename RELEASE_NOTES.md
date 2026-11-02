RPP 0.9.1 fixes data-integrity, editing and export problems found during a source
and interactive audit of 0.9.0. All platforms continue to use the same Rust code.

Fixes:
- Atomic project, recovery, preferences and export writes, with unique temporary
  files and preservation of the previous project if backup/replacement fails.
- Bounded disk reads, ZIP expansion and legacy decompression; shared raster
  decoder limits and a cumulative decoded-image memory budget.
- Locked objects and layers are protected from deletion, nudging, splitting and
  stale-selection drags. The unlock control remains visible above properties.
- Group actions preserve older undo entries. Cancelled drags restore their
  preview, and new/open/undo reset every transient tool state.
- Invalid edits roll back, orphan layer assignments are removed, text limits
  preserve UTF-8, and malformed settings cannot introduce invalid geometry.
- Automatic room metadata retains its layer after geometry changes. Detection
  has a bounded work budget and retains existing rooms if that budget is exceeded.
- Exports follow layer order, hide openings whose parent wall is hidden, include
  thick wall extents and handle XML control characters safely.
- Layer lookups are indexed for rendering/export, and layer opacity is applied
  once to the whole canvas layer.
- Save/recovery errors are visible in the status bar. Native application exit
  writes recovery for committed unsaved changes.
- Updated ZIP, browser integration, SVG/text rendering and YAML dependencies;
  removed unused legacy dependency features and withdrawn dependency versions.
- Release packages verify binary architecture. Publication uploads to a draft
  first and only publishes after all platform and dependency checks succeed.

See AUDIT.md for verification and practical limits. Windows binaries are unsigned;
macOS apps have an ad-hoc signature and are not Apple notarized. Native pinch
availability depends on the OS, driver and windowing backend.
