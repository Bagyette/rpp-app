# RPP project container and RPP-PLAN specification

This specification describes the RPP 0.9.1 project reader/writer and the current
working-tree recovery implementation. Portable projects use two simple layers:

1. **RPP project container** — the ZIP-compatible binary file carrying the
   `.rpp` extension;
2. **RPP-PLAN** — the readable UTF-8 language stored as `plan.data` inside the
   container.

The outer file keeps images beside the plan. The inner language keeps the
geometry readable. RPP-PLAN is always written in English: changing the
interface language never turns `WALL` into a translated keyword.

If you only want to write a small plan by hand, read sections 2, 3 and 9 first.
The instruction reference in section 4 has the exact field order when you need
it.

## 1. Project container

### 1.1 Structure

An `.rpp` project is a standard ZIP-compatible archive:

```text
house.rpp
├── plan.data
└── assets/
    ├── asset-floor-reference.jpg
    ├── asset-logo.png
    └── asset-symbol.webp
```

`plan.data` is required. The `assets/` directory is only needed when the plan
references imported images.

RPP reads the archive directly in memory and does not extract entries to a
temporary directory.

To inspect a project manually:

1. make a copy of the `.rpp` file;
2. rename the copy from `.rpp` to `.zip`;
3. open it with a normal archive tool;
4. open `plan.data` with a UTF-8 text editor.

Editing a project this way is supported as long as the final archive follows
this specification.

### 1.2 Required paths

| Entry | Required | Purpose |
|---|---:|---|
| `plan.data` | Yes | RPP-PLAN source |
| `assets/<id>.<extension>` | When declared | Imported image bytes |

An asset path is derived from its `ASSET` instruction. For example:

```text
ASSET asset-logo png company-logo
```

requires:

```text
assets/asset-logo.png
```

Path names are never taken from untrusted free-form archive text. Identifiers
and extensions are validated before RPP constructs an asset path.

### 1.3 ZIP compression

- `plan.data` uses ZIP Deflate.
- BMP assets use ZIP Deflate.
- PNG, JPEG, GIF, and WebP assets are stored without another ZIP compression
  pass because those formats are already compressed.

Image processing occurs when an image is imported, not each time the project
is saved.

### 1.4 Built-in assets

The stable asset identifier below is provided by the application:

```text
builtin-emergency-exit
```

It may be referenced by an `IMAGE` instruction without an `ASSET` declaration
or matching file in `assets/`.

Built-in bytes are never copied into the project. This keeps every instance
small and allows the same bundled symbol to be reused across projects. If the
running application cannot resolve an image asset, it renders a neutral
missing-image placeholder rather than substituting unrelated content.

### 1.5 Readable by design

The `.rpp` extension is a file association, not encryption. Rename a copy to
`.zip` and you can inspect the whole project. That is intentional.

## 2. RPP-PLAN fundamentals

### 2.1 Encoding

- The file is UTF-8 text.
- Line endings may be Windows `CRLF` or Unix `LF`.
- Empty lines are ignored.
- Instructions and enumerated values are case-sensitive.
- Ordinary instruction tokens are separated by one or more whitespace characters.
- For the four JSON-string instructions (`TEXT`, `OBJECT_NAME`, `ROOM_NAME`,
  `LAYER`), put an ASCII space immediately after the keyword; the reader uses
  that prefix to distinguish their string/comment handling.
- Decimal numbers use a period, for example `12.5`.
- `NaN`, positive infinity, and negative infinity are rejected.

### 2.2 Header

The first non-empty, non-comment instruction must be:

```text
RPP-PLAN 2
```

The RPP-PLAN version is independent from the application version. RPP 0.9.1
writes version 2.

The legacy header below is still accepted:

```text
RPP-PLAN 1
```

Saving a loaded legacy file writes a current packaged project.

### 2.3 Units

One unit declaration is required:

```text
UNITS cm
```

Accepted unit tokens and their conversion factors are:

| Token | Meaning | Centimeter factor |
|---|---|---:|
| `mm` | millimeters | 0.1 |
| `cm` | centimeters | 1 |
| `m` | meters | 100 |
| `in` | inches | 2.54 |
| `ft` | feet | 30.48 |

Every coordinate, length, width, thickness, text size, compass size, and arrow
size is converted to canonical centimeters when loaded. Dimensionless values,
such as opening positions and lock flags, are not scaled.

RPP always writes `UNITS cm`, even if the interface currently displays another
unit. This makes saved output predictable and avoids cumulative conversions.

### 2.4 Coordinate system

- `(0, 0)` is an unrestricted origin.
- X increases to the right.
- Y increases downward.
- Negative coordinates are valid.
- Rotations are expressed in degrees unless an instruction explicitly uses
  orientation steps.

### 2.5 Comments

For ordinary instructions, `#` starts a comment:

```text
WALL north 0 0 400 0 20 exterior false 34383b # exterior wall
```

`TEXT`, `OBJECT_NAME`, `ROOM_NAME`, and `LAYER` lines are special because their JSON string may
contain `#`:

```text
TEXT paint-note 100 50 12 0 false 34383b false "Paint #145ab4"
OBJECT_NAME desk-1 "Desk #1"
```

The complete remainder of these lines must be valid JSON string content.
Do not append a trailing `# comment` after any of these four instructions.

Comments are accepted when reading handwritten files. RPP currently rewrites
`plan.data` when saving and does **not** preserve original comments, blank-line
layout, or instruction ordering.

### 2.6 Identifiers

Identifiers are used by all persistent plan items and assets.

Rules:

- length: 1 to 128 ASCII bytes;
- allowed characters: `A-Z`, `a-z`, `0-9`, `.`, `-`, `_`;
- no spaces;
- no accented characters;
- no quotes or slashes.

Valid examples:

```text
north-wall
bedroom_2.window
asset-logo-01
```

Invalid examples:

```text
north wall
fenêtre
../image
```

All plan-item identifiers share one global namespace. Walls, lines, doors,
windows, objects, text, areas, rooms, compass roses, arrows, and image instances
cannot reuse another plan item's identifier. Layer identifiers have a separate
namespace and must be unique among layers.

Asset identifiers are validated separately and must be unique among assets.

### 2.7 Booleans

Booleans are lowercase:

```text
true
false
```

Values such as `True`, `FALSE`, `1`, and `0` are invalid.

### 2.8 Colors

Colors are six hexadecimal RGB digits without `#`:

```text
34383b
ff0000
00a050
145ab4
```

Uppercase and lowercase hexadecimal digits are accepted. RPP writes lowercase
colors.

### 2.9 JSON strings

Free-form text is represented as a JSON string:

```text
"Living room"
"Quote: \"hello\""
"First line\nSecond line"
"C:\\Plans\\house"
```

This applies to `TEXT`, `OBJECT_NAME`, `ROOM_NAME`, and layer names. Standard
JSON escaping rules must be used.

## 3. Complete example

```text
RPP-PLAN 2
# All values below use centimeters.
UNITS cm

WALL north 0 0 400 0 20 exterior false 34383b
WALL east 400 0 400 300 20 exterior false 34383b
WALL south 400 300 0 300 20 exterior false 34383b
WALL west 0 300 0 0 20 exterior false 34383b

LINE reference-axis 0 350 400 350 false 145ab4

DOOR entrance north 0.5 90 single left inward false 464646
WINDOW garden-window south 0.5 120 bay false 2d7391

OBJECT dining-table table 200 170 140 80 0 false e1e4e2 5a5a5a
OBJECT_NAME dining-table "Family table"

TEXT room-label 200 120 18 0 false 34383b true "Living room"
AREA floor-zone false d6e6d2 466e46 0 0 400 0 400 300 0 300
ROOM living-room automatic living-room false true dce6ef 0 0 400 0 400 300 0 300
ROOM_NAME living-room "Living room"
LAYER default true false 1 "Default"
LAYER furniture true false 0.85 "Furniture"
ASSIGN dining-table furniture
COMPASS north-indicator 460 70 80 0 false 34383b
ARROW evacuation 80 80 120 24 solid 0 false 169c35

ASSET asset-logo png company-logo
IMAGE logo-1 asset-logo 200 220 60 40 0 false
IMAGE exit-1 builtin-emergency-exit 360 250 40 40 0 false
```

## 4. Instruction reference

### 4.1 `WALL`

Full syntax:

```text
WALL <id> <x1> <y1> <x2> <y2> <thickness> <type> <locked> <color>
```

Short syntax:

```text
WALL <id> <x1> <y1> <x2> <y2>
```

Intermediate syntax without color:

```text
WALL <id> <x1> <y1> <x2> <y2> <thickness> <type> <locked>
```

Fields:

| Field | Type | Meaning |
|---|---|---|
| `x1`, `y1` | number | Start point |
| `x2`, `y2` | number | End point |
| `thickness` | positive number | Physical wall thickness |
| `type` | enum | `interior` or `exterior` |
| `locked` | boolean | Prevent accidental editing |
| `color` | RGB | Wall color |

Defaults used by the short form:

```text
thickness = 14
type = interior
locked = false
color = 34383b
```

The start and end points must differ. Horizontal, vertical, diagonal, and
negative-coordinate walls are valid.

Example:

```text
WALL partition 40 60 280 190 10 interior false 4b4b4b
```

### 4.2 `LINE`

Syntax:

```text
LINE <id> <x1> <y1> <x2> <y2> <locked> [color]
```

Default color:

```text
4b4b4b
```

A line is an independent annotation. It does not receive doors or windows and
must have non-zero length.

Example:

```text
LINE center-axis -50 100 450 100 false 145ab4
```

### 4.3 `DOOR`

Full syntax:

```text
DOOR <id> <wall-id> <position> <width> <type> <hinge> <swing> <locked> <color>
```

Short syntax:

```text
DOOR <id> <wall-id> <position> <width>
```

Intermediate syntax without color:

```text
DOOR <id> <wall-id> <position> <width> <type> <hinge> <swing> <locked>
```

Fields:

| Field | Accepted values | Meaning |
|---|---|---|
| `wall-id` | wall identifier | Receiving wall |
| `position` | `0` to `1` | Center along the wall |
| `width` | positive number | Opening width |
| `type` | `single`, `double`, `sliding` | Door representation |
| `hinge` | `left`, `right` | Hinge side |
| `swing` | `inward`, `outward` | Swing side |
| `locked` | boolean | Prevent movement/editing |
| `color` | RGB | Door line color |

Defaults:

```text
type = single
hinge = left
swing = inward
locked = false
color = 464646
```

`position = 0` is the wall start and `position = 1` is the wall end. Because
the value describes the opening center, its actual valid range is narrower:
the complete width must fit inside the wall.

Example:

```text
DOOR entrance north 0.35 90 double right inward false 464646
```

### 4.4 `WINDOW`

Syntax:

```text
WINDOW <id> <wall-id> <position> <width> <type> <locked> [color]
```

Types:

```text
standard
bay
```

Default color:

```text
2d7391
```

The referenced wall must exist and the complete window width must fit inside
it. The graphical editor prevents opening overlaps during normal placement.
The file validator guarantees references and containment but does not currently
reject every possible door/window overlap in a handwritten file.

Example:

```text
WINDOW kitchen-window east 0.6 140 standard false 2d7391
```

### 4.5 `OBJECT`

Full syntax:

```text
OBJECT <id> <type> <x> <y> <width> <height> <rotation> <locked> <fill-color> <stroke-color>
```

Short syntax:

```text
OBJECT <id> <type> <x> <y> <width> <height> <rotation> <locked>
```

Object types:

```text
rectangle
bed
table
sofa
wardrobe
desk
```

Defaults:

```text
fill-color = e1e4e2
stroke-color = 5a5a5a
```

`x` and `y` describe the object center. Width and height must be positive.
Rotation is clockwise in degrees.

Example:

```text
OBJECT desk-1 desk 120 200 140 70 90 false e1e4e2 5a5a5a
```

### 4.6 `OBJECT_NAME`

Syntax:

```text
OBJECT_NAME <object-id> <JSON string>
```

Example:

```text
OBJECT_NAME desk-1 "Main workstation"
```

The referenced `OBJECT` must already have appeared earlier in the file.
Names are optional and may contain spaces, Unicode, quotes, and line breaks
through JSON escaping. The encoded name is limited to 512 bytes.

An object name changes only the displayed label. It does not create a new
object type.

### 4.7 `TEXT`

Full syntax:

```text
TEXT <id> <x> <y> <size> <rotation> <locked> <color> <bold> <JSON string>
```

Accepted shorter forms:

```text
TEXT <id> <x> <y> <size> <rotation> <locked> <JSON string>
TEXT <id> <x> <y> <size> <rotation> <locked> <color> <JSON string>
```

Defaults:

```text
color = 34383b
bold = false
```

The text position is its center. Size uses the active file unit and must
convert to between 1 and 10,000 cm. Rotation is clockwise in degrees.

The only explicitly supported family is Arial, loaded from Windows at runtime.
If Arial is unavailable, egui's built-in proportional fallback is used.

Text content is limited to 10,000 UTF-8 bytes.

Examples:

```text
TEXT title 200 50 20 0 false 34383b true "Ground floor"
TEXT note 200 80 12 -15 false b42846 false "Paint: #f4efe6"
TEXT multiline 200 120 12 0 false 34383b false "First line\nSecond line"
```

### 4.8 `AREA`

Syntax:

```text
AREA <id> <locked> <fill-color> <stroke-color> <x1> <y1> <x2> <y2> <x3> <y3> [...]
```

An area:

- contains between 3 and 10,000 points;
- is closed implicitly from the last point back to the first;
- may be concave;
- must produce a positive, finite surface;
- uses independent fill and outline colors.

Do not repeat the first point at the end unless that repeated vertex is
deliberately wanted in the polygon data.

Example:

```text
AREA room-fill false d6e6d2 466e46 0 0 400 0 400 300 0 300
```

RPP calculates the surface using the shoelace formula and displays it in the
selected interface unit.

### 4.9 `ROOM` and `ROOM_NAME`

Room geometry uses:

```text
ROOM <id> <source> <type> <locked> <fill-visible> <fill-color> <x1> <y1> <x2> <y2> <x3> <y3> [...]
```

`source` is `automatic` or `manual`. Automatic rooms are detected from the
bounded faces of the wall graph. Their stored polygon is refreshed when wall
geometry changes. Manual rooms are user-defined polygons and do not require a
closed wall contour.

Supported room types are:

```text
custom living-room bedroom kitchen bathroom hallway office garage storage
```

The optional display name is a separate JSON string:

```text
ROOM_NAME <room-id> <JSON string>
```

Example:

```text
ROOM room-1 manual office false true dce6ef 0 0 400 0 400 300 0 300
ROOM_NAME room-1 "Studio #1"
```

Surface and perimeter are derived from the polygon and are not stored as
separate numbers.

### 4.10 `LAYER` and `ASSIGN`

Syntax:

```text
LAYER <id> <visible> <locked> <opacity> <JSON name>
ASSIGN <item-id> <layer-id>
```

Opacity is from `0` to `1`. Layers are drawn in stored order: the first is at the
bottom and the last is at the top. The text reader starts with `default` at the
bottom and appends each newly declared layer. A repeated `LAYER` identifier
updates that layer in place without moving it. A locked layer protects its items from
editing, deletion, nudging, splitting, and dragging. A hidden layer is omitted
from the editor and exports. Openings also follow their parent wall's visibility.

A valid plan must include a layer whose identifier is `default`. The text reader
creates it before parsing instructions, even when only other layers are declared.
A `LAYER default ...` instruction updates its properties. JSON/legacy binary
readers validate their stored layer list and reject a missing default layer.

Items without an `ASSIGN` instruction belong to the `default` layer.
Each item may have at most one assignment. Deleting a layer in the application
moves its items back to the default layer.

Example:

```text
LAYER default true false 1 "Default"
LAYER furniture true false 0.8 "Furniture"
ASSIGN table-1 furniture
```

### 4.11 `COMPASS`

Syntax:

```text
COMPASS <id> <x> <y> <size> <rotation> <locked> <color>
```

The position is the center. Size must convert to between 1 and 100,000 cm.
Rotation is clockwise from geographic north.

Example:

```text
COMPASS north-indicator 460 70 80 15 false 34383b
```

### 4.12 `ARROW`

Syntax:

```text
ARROW <id> <x> <y> <length> <width> <style> <orientation-step> <locked> <color>
```

Styles:

```text
open
open-bold
hollow
solid
```

Orientation is an integer from `0` to `63`. Each step is:

```text
360 / 64 = 5.625 degrees
```

Direction mapping:

| Step | Direction |
|---:|---|
| `0` | right |
| `16` | down |
| `32` | left |
| `48` | up |
| `63` | 354.375 degrees |

Length must convert to between 1 and 1,000,000 cm. Width must convert to between
0.1 and 1,000,000 cm.

Examples:

```text
ARROW open-arrow 100 100 120 20 open 0 false 34383b
ARROW north-west 200 200 80 16 solid 40 false 145ab4
```

Arrows are procedural vector geometry. No arrow image is embedded.

### 4.13 `ASSET`

Syntax:

```text
ASSET <id> <extension> <name>
```

Accepted extensions:

```text
png
jpg
jpeg
gif
bmp
webp
```

`name` is a sanitized identifier, not a quoted display string. It follows the
same 128-byte and character rules as other identifiers.

Example:

```text
ASSET asset-reference jpg floor-reference
```

Matching archive entry:

```text
assets/asset-reference.jpg
```

The bytes must describe a supported, non-empty image and must agree with a
decodable image format. RPP validates the data rather than trusting only the
extension.

### 4.14 `IMAGE`

Syntax:

```text
IMAGE <id> <asset-id> <x> <y> <width> <height> <rotation> <locked>
```

The position is the image center. Width and height must be positive. Rotation
is clockwise in degrees.

Multiple `IMAGE` instructions may reference one `ASSET`:

```text
IMAGE logo-large asset-logo 100 100 80 40 0 false
IMAGE logo-small asset-logo 300 100 40 20 15 false
```

This reuses the same embedded bytes.

Built-in example:

```text
IMAGE exit-sign builtin-emergency-exit 200 200 40 40 0 false
```

No `ASSET` declaration is required for that stable built-in identifier.

## 5. Declaration order and references

Most relationships are validated after the complete file is parsed:

- doors and windows may reference a wall declared elsewhere in the file;
- image instances may reference an asset declared elsewhere in the file.

`OBJECT_NAME` and `ROOM_NAME` are exceptions: their referenced item must already
have been declared because the name is applied immediately while parsing.

For readability, the writer uses this order:

1. walls;
2. lines;
3. doors;
4. windows;
5. objects and their names;
6. assets;
7. image instances;
8. text;
9. areas;
10. rooms and their names;
11. layers;
12. layer assignments;
13. compass roses;
14. arrows.

Handwritten files do not otherwise need to preserve this order.

## 6. Validation behavior

RPP validates a project before accepting it and before writing it.

The validator rejects, among other conditions:

- unsupported headers or units;
- unknown instructions;
- malformed field counts;
- invalid JSON strings;
- duplicate plan identifiers;
- duplicate asset identifiers;
- invalid colors, booleans, enums, or numbers;
- `NaN` and infinite numbers;
- zero-length walls and lines;
- non-positive dimensions;
- doors and windows outside their wall;
- references to missing walls or assets;
- invalid or empty image data;
- degenerate areas;
- degenerate rooms or invalid room types;
- missing layers, duplicate layer assignments, or opacity outside `0..1`;
- arrow orientation outside `0..63`;
- values exceeding implementation limits.

Text syntax errors include a one-based line number.

## 7. Security and complexity limits

RPP 0.9.1 applies these limits:

| Resource | Limit |
|---|---:|
| Complete input project | 256 MiB |
| Total expanded ZIP content | 256 MiB |
| Total decoded pixels of all embedded assets (RGBA) | 256 MiB |
| `plan.data` | 64 MiB |
| Individual asset | 32 MiB |
| Embedded assets | 256 |
| Archive entries | 257 |
| Decoded image width | 8,192 px |
| Decoded image height | 8,192 px |
| Image decoder allocation | 256 MiB |
| Items of each plan type | 100,000 |
| Points in one area | 10,000 |
| Points in one room | 10,000 |
| Layers | 1,000 |
| Layer assignments | 500,000 |
| Identifier length | 128 bytes |
| Object name | 512 bytes |
| Text content | 10,000 bytes |

Many geometric values are additionally restricted to finite, reasonable
ranges. These are parser safety limits, not recommended practical project
sizes. The interface may become inconvenient before a theoretical maximum is
reached.

Archive entries are read through known paths and are never extracted using
arbitrary stored paths, preventing path traversal during normal loading.

## 8. Compatibility and migration

RPP 0.9.1 can open:

- packaged `.rpp` ZIP containers containing `RPP-PLAN 2`;
- standalone readable `RPP-PLAN 2` text;
- readable `RPP-PLAN 1` text;
- the early JSON plan representation;
- the legacy compressed `RPPBIN` representation.

Legacy inputs are marked for conversion. Saving writes the current
ZIP-compatible container and canonical RPP-PLAN 2 text.

RPP writes complete current fields even when a shorter legacy instruction was
read. It also normalizes:

- units to `cm`;
- booleans to lowercase;
- colors to lowercase hexadecimal;
- numeric formatting through Rust's finite `f32` representation;
- instruction order to the writer's standard order.

Never overwrite the only copy of a handcrafted or legacy file without a
backup. The application normally creates a sibling `.rpp.bak` file before
replacing an existing project.

## 9. Writing a project by hand

You do not need RPP to generate a valid project. A text editor is enough for a
plan without imported images, and a normal ZIP tool handles the rest.

### 9.1 Text-only plan

Create `plan.data`:

```text
RPP-PLAN 2
UNITS cm

WALL wall-1 0 0 400 0 14 interior false 34383b
WALL wall-2 400 0 400 300 14 interior false 34383b
LINE note-line 0 340 400 340 false 145ab4
TEXT note 200 360 14 0 false 34383b false "Handwritten RPP-PLAN"
```

RPP can open this standalone text directly. Saving it converts it into a
packaged `.rpp` project.

### 9.2 Packaged project

1. Create a work directory.
2. Put the readable source at `plan.data`.
3. Create `assets/` if the plan declares imported assets.
4. Add every file using `assets/<asset-id>.<extension>`.
5. ZIP the **contents** of the work directory, not the directory itself.
6. Rename the resulting `.zip` file to `.rpp`.
7. Keep the original work directory until RPP opens the project successfully.

The archive root must contain `plan.data` directly:

```text
correct:
project.rpp/plan.data

incorrect:
project.rpp/my-project/plan.data
```

### 9.3 When RPP rejects the file

If RPP reports a syntax line:

1. extract or open `plan.data`;
2. go to the reported one-based line number;
3. verify the exact instruction field count;
4. check lowercase booleans and enums;
5. check IDs for spaces or accented characters;
6. check JSON escaping for `TEXT` and `OBJECT_NAME`;
7. verify wall and asset references;
8. confirm that every dimension is positive where required.

For a container-level rejection, verify `plan.data`, asset paths, actual image
bytes, and file-size limits.

## 10. Format evolution rules

The format is still young, but changing it casually would make handwritten
plans miserable to maintain. Future changes therefore follow these rules:

- increment the RPP-PLAN header version only for a real grammar or semantic
  change;
- keep application versioning separate from file-language versioning;
- document every new instruction and migration;
- preserve old readers where practical;
- never silently reinterpret an existing token with incompatible semantics;
- retain English-only keywords;
- keep the writer deterministic and readable;
- validate references and complexity before replacing a user's file;
- add parser, writer, validation, migration, and round-trip tests.


### 10.1 Reader hardening in 0.9.1

Readers bound disk reads before decoding, cap cumulative ZIP expansion, limit
legacy zlib expansion to the declared uncompressed size, and reject duplicate
asset descriptors. Images must fit both the per-image dimensions/allocation
limits and the cumulative decoded-image budget. A valid plan must contain the
`default` layer. Non-finite or out-of-bounds polygon coordinates are rejected.

Automatic room detection has a separate computational budget: 4,096 input walls,
200,000 candidate intersection pairs and 10,000 split segments. If a plan exceeds
that budget, existing room metadata is retained and the editor displays a warning;
the document can still be opened and edited.

## 11. Recovery sessions (internal, not a portable project)

The current working tree stores recovery in the platform's `autosave.rpp` path,
but the new session envelope is **not** the ZIP project container described
above. Do not rename it to ZIP or use it as a normal project file. An explicit
Save still writes the unchanged ZIP/RPP-PLAN 2 format.

The recovery envelope is a single atomically replaced file:

| Component | Encoding |
|---|---|
| Magic | ASCII `RPP-RECOVERY-1` followed by one LF byte |
| Metadata length | Unsigned 32-bit little-endian byte count |
| Metadata | UTF-8 JSON, at most 64 KiB |
| Project payload | Normal packaged `.rpp` bytes, present only for an unsaved session |

Metadata has these fields:

```json
{"source": "C:\\Plans\\house.rpp", "unsaved": true}
```

`source` is the original project path, or `null` for an untitled plan. It is
session-local and may be platform-specific; it is never written into a portable
project. `unsaved: false` is a clean-session marker with no project payload.
Saved, new, explicitly discarded, and undo-to-saved sessions write that marker
so a later launch cannot resurrect an already saved or discarded plan.

Only unsaved sessions are recovered. Recovery preserves the original path and
compares restored contents with the saved project when that file is readable.
If the contents already match, the recovered document is clean and the recovery
marker is cleared. If the original file is missing or differs, recovered changes
remain unsaved. Old recovery files containing a plain project are still accepted;
they lack an original path and are treated as unsaved for compatibility.

Recovery jobs are serialized with foreground file operations. At most one pending
recovery snapshot is retained; newer edits replace queued recovery work. Normal
application exit flushes the queue. Forced termination cannot guarantee that a
pending write has finished. The total envelope read limit is the 256 MiB project
limit plus 64 KiB of metadata, the magic, and the four-byte length field.
