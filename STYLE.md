# Rust style and review

RPP follows the public Rust style conventions and the applicable recommendations
in [Google's Rust Style Guide](https://google.github.io/styleguide/rustguide.html).
The Chrome/Bazel and Fuchsia-only requirements do not apply to this Cargo app.

Checks enforced on Windows, Linux and macOS:

- Stable `rustfmt`, Rust 2024, a target width of 100 columns.
- Clippy with warnings treated as errors, no wildcard imports, and no `unwrap`.
- Explicit imports grouped by source module, meaningful conversion names,
  `expect` messages for invariant failures, and no application `unsafe` code.
- Regression tests for logical keyboard layouts, command modifiers, navigation,
  persistence paths, file compatibility and exports.

The shared platform and input modules isolate operating-system integration from
the plan model. Platform differences should be handled through standard crates
or narrow platform conditions, without forking the editor implementation.

Review decisions for the existing application:

- The editor's `app` module retains its implementation and child inspector/render
  modules. This is an explicit exception to Google's leaf-module-only layout;
  reorganizing the entire editor is separate from the portability release.
- The egui API is a large related group, so its UI types may remain qualified.
- Small private geometry/model helpers with unambiguous names remain directly
  imported in the editor. General third-party functions use module qualification.
- Shared `Arc` values remain where egui requires them or where undo history shares
  image assets, avoiding copies of potentially large images.
- Long generated SVG literals may exceed 100 columns where splitting the literal
  would obscure the output format; automatic formatting remains enabled.

These decisions are documented exceptions, not a claim that every historical
line meets every Google recommendation. New code should keep dependencies and
error behavior explicit and use the narrowest practical visibility.
