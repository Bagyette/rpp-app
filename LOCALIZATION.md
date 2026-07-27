# RPP localization guide

RPP keeps every interface translation in one catalog:

```text
native/lang.yaml
```

The catalog is embedded into the executable at compile time. The portable
application does not need external language files at runtime.

## Included languages

| Code | Language | Role |
|---|---|---|
| `en-US` | English (US) | Default, source, and fallback |
| `fr-FR` | French | Complete translation |
| `es-ES` | Spanish | Complete translation |

The selected language is stored locally in:

```text
%LOCALAPPDATA%\RPP\language.txt
```

Changing the language affects only the interface. It does not translate or
rewrite RPP-PLAN keywords, object identifiers, filenames, or user-entered text.

## Catalog structure

```yaml
default_language: en-US
languages:
  en-US:
    name: English (US)
    strings:
      action.save: Save
      status.saved: "Saved: {path}"
  fr-FR:
    name: Français
    strings:
      action.save: Enregistrer
      status.saved: "Enregistré : {path}"
  es-ES:
    name: Español
    strings:
      action.save: Guardar
      status.saved: "Guardado: {path}"
```

Each language entry contains:

- `name`: the native label displayed in the language selector;
- `strings`: a mapping from stable source keys to translated values.

## Key rules

Every language must contain exactly the same keys.

Examples of stable keys:

```text
action.save
dialog.open_title
property.width
status.export_error
tool.arrow_instruction
```

Keys:

- are always English;
- describe meaning rather than visual position;
- must not be translated;
- should remain stable once released;
- use a short dotted namespace.

The automated test `every_language_has_identical_keys` fails if English,
French, and Spanish no longer have identical key sets.

RPP still has an English fallback for defensive robustness. A missing key in a
selected language displays the `en-US` value; a key missing from English too is
displayed literally. The fallback is not an excuse to commit an incomplete
translation.

## Placeholders

Some values contain runtime placeholders:

```yaml
status.saved: "Saved: {path}"
status.export_error: "{format} export failed: {error}"
status.items_selected: "{count} items selected"
```

Translate the surrounding sentence, but preserve each placeholder name and its
curly braces exactly.

Correct:

```yaml
status.saved: "Enregistré : {path}"
```

Incorrect:

```yaml
status.saved: "Enregistré : {chemin}"
```

Common placeholders currently include:

```text
{path}
{error}
{format}
{version}
{line}
{count}
{distance}
{unit}
{scale}
{limit}
{x}
{y}
```

Pluralized status strings additionally use count and suffix placeholders such
as `{walls}` and `{wall_suffix}`.

## YAML quoting

Quote a translation when it contains characters that YAML may interpret:

- a colon followed by a space;
- leading or trailing whitespace;
- braces in an ambiguous context;
- `#`;
- values resembling booleans or numbers;
- escape sequences.

Safe example:

```yaml
format.unsupported_version: "Unsupported RPP version: {version}"
```

Use spaces for indentation. Do not use tabs.

## Adding a language

1. Copy the complete `en-US` language entry.
2. Choose an IETF BCP 47 language tag, for example `de-DE`.
3. Set `name` to the language's native name.
4. Translate every value under `strings`.
5. Preserve keys and placeholders exactly.
6. Keep keyboard-shortcut letters accurate if a translated instruction
   mentions one.
7. Run formatting and tests.
8. Build the application and inspect the interface for truncation.

Example:

```yaml
  de-DE:
    name: Deutsch
    strings:
      app.ready: Bereit
      action.save: Speichern
      # Continue with every existing key.
```

## Updating interface text

When adding a new user-facing string:

1. create an English key in `en-US`;
2. add the identical key to `fr-FR` and `es-ES`;
3. translate it in context;
4. replace hard-coded UI text in Rust with `localizer.text(...)` or
   `localizer.format(...)`;
5. run the tests.

User-facing application strings belong in `lang.yaml`. Source-code identifiers,
comments, tests, internal type names, and RPP-PLAN syntax remain English.

Some technical labels intentionally remain universal, including:

```text
RPP
SVG
PNG
PDF
mm
cm
m
in
ft
Ctrl
Shift
```

## RPP-PLAN is never localized

These file-language tokens remain English in every interface language:

```text
RPP-PLAN
UNITS
WALL
LINE
DOOR
WINDOW
OBJECT
OBJECT_NAME
TEXT
AREA
COMPASS
ARROW
ASSET
IMAGE
```

Enumerated file values also remain English:

```text
interior
exterior
single
double
sliding
left
right
inward
outward
standard
bay
rectangle
bed
table
sofa
wardrobe
desk
open
open-bold
hollow
solid
true
false
```

This separation is deliberate: localized UI labels are presentation, while
RPP-PLAN is a stable exchange language.

## Validation commands

From the `native` directory:

```powershell
cargo test localization
cargo test
cargo build --release
```

The localization tests verify:

- English is the default language;
- translation key parity;
- Spanish core strings resolve correctly;
- missing selected-language strings fall back to English.

After automated tests, manually check:

- the top language selector;
- the Settings language selector;
- tool labels and instructions;
- the Properties panel;
- file dialogs and error messages;
- long translated labels at normal Windows scaling.
