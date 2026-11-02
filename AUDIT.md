# RPP 0.9.1 audit — 2026-11-02

Scope: the shared Rust editor, inspectors and layer/room controls; project
serialization and legacy readers; raster imports; SVG/PNG/PDF exports; platform
paths and fonts; logical keyboard and mouse/trackpad input; release packaging and
GitHub Actions. This is a source and regression review, not formal verification.

## Findings corrected

| Area | Finding | Change |
|---|---|---|
| Project writes | Fixed temporary names could overwrite an unrelated `.rpp.tmp`; moving the original before replacement left a failure gap | Unique same-directory temporary files, synchronized replacement and a copied backup; original remains readable on failure |
| Recovery/settings/exports | Direct writes could leave truncated output | Atomic replacement; recovery write failures stay visible |
| File input | Size checks followed unbounded disk reads | Bounded regular-file reads before parsing; special files such as FIFOs are rejected before opening |
| Compression | Legacy zlib output was unbounded; ZIP entries had individual but no cumulative expansion budget | Declared-size decompression limit, total expanded-size cap and duplicate descriptor rejection |
| Raster import | Recompression decoded through an unrestricted path; aggregate decoded pixel memory was unbounded | Shared limited decoder, dimensions checks and per-project decoded-image budget |
| Data validation | Missing assets/default layer and extreme polygon coordinates could enter the editor | Validation rejects those inputs; invalid internal edits roll back |
| Locks | Keyboard deletion and stale selections could bypass item/layer locks | Shared edit checks and an accessible central lock/unlock control |
| Undo | Recursive group actions could evict multiple old undo entries | One history transaction per group operation |
| Tool state | New/open/undo left some drafts/drags behind; Escape could leave an uncommitted preview in the model | Full transient-state reset and preview rollback |
| Rooms | Recomputed automatic rooms lost layer assignment; dense graphs could exhaust CPU/memory | Metadata/layer transfer, spatial node lookup and explicit complexity budgets |
| Numeric geometry | Polygon area/centroid calculations lost precision at large coordinate offsets | Translated double-precision accumulation |
| Text/settings | Large UTF-8 strings or malformed numeric settings could make an edited project invalid | UTF-8 boundary limits, numeric sanitization and commit validation |
| Feedback | Internal error/status messages were not rendered in the status bar | Visible bounded status text and full diagnostic tooltip |
| Exports | Drawing order differed from canvas layers, thick walls could be clipped, and hidden-wall openings could leave invalid XML | Layer-ordered export, expanded bounds, parent visibility checks and safe XML text |
| Rendering | Repeated scans of layer assignments slowed painting; opacity application varied by element | Per-frame layer index and one opacity application per layer |
| Input | Horizontal tilt-wheel input was ignored; panning/overlay input could interrupt drag completion | Horizontal scroll handling and drag-aware navigation routing |
| Dependencies | Four published advisories affected the locked `zip`/`webbrowser` versions | Updated ZIP/browser dependencies; also replaced abandoned YAML and text-shaping dependencies, disabled unused postcard defaults and updated withdrawn clipboard version |
| Packaging | Requested architecture could differ from executable architecture | Verify PE/ELF/Mach-O header before naming/package creation |
| Publication | A superseded commit or incomplete asset upload could be published | Current-main check, draft-first upload and dependency-audit gate |

The three ZIP advisories concern filesystem extraction APIs that RPP does not
call: project entries are read in memory. Updating still removes those vulnerable
versions. The browser advisory requires a caller-controlled URL and a relevant
Unix browser environment; the editor has no project-URL opening feature.

References: [ZIP advisory](https://rustsec.org/advisories/RUSTSEC-2026-0367.html),
[browser advisory](https://rustsec.org/advisories/RUSTSEC-2026-0257.html).

## Verification

- Rust regression tests cover project round trips, malformed/oversized input,
  preservation of backups, failed writes, locked editing, group history, drag
  cancellation, room metadata, exports, logical keyboard layouts and navigation.
- Stable rustfmt, strict Clippy, RustSec dependency audit, Actions workflow lint
  and package architecture/signature checks.
- Interactive macOS checks use a disposable plan through Computer Use. The
  original locked-wall deletion defect was reproduced before correction. In the
  corrected application, locking prevented deletion/nudging, unlocking allowed
  movement, and scrolling, zoom buttons, Fit and Undo worked without creating
  extra geometry. Native Save and Open were verified after the user accepted the
  macOS file-access prompt. The disposable saved project was moved to
  `/tmp/rpp-audit-2026-11-02.rpp` after returning the editor to a blank plan.
- GitHub Actions checks Windows x64, Linux x64, macOS arm64 and macOS x64 before
  publishing release assets. Actual run results are available with the release.

## Practical limits

- The Mac package is ad-hoc signed, without Apple notarization; Windows is unsigned.
- Native pinch support depends on drivers/backend events. Synthetic input tests
  do not prove behavior on every physical keyboard or trackpad.
- The Linux binary targets Ubuntu 24.04-compatible runtime libraries; this is not
  an AppImage or a statically linked binary for every Linux distribution.
- Large project serialization/recovery and some geometry operations remain
  synchronous on the UI thread. Resource/work budgets prevent the identified
  unbounded paths; maximum-size projects can still pause the UI.
- The existing editor architecture and approximate text/measurement extents are
  retained. Google-style exceptions are documented in STYLE.md.
