RPP 0.9.0 adds native macOS and Linux builds alongside Windows, using the same
Rust editor code and the existing `.rpp` project format.

Changes:
- Native user directories for preferences, language and recovery, preserving the
  existing Windows storage location.
- System font lookup and bundled fallbacks shared by canvas text and PNG/PDF
  exports, without hardcoded Windows font paths.
- Trackpad pan, pointer-anchored pinch zoom, middle-button and Space+drag pan,
  Shift+wheel horizontal pan, Alt/Option+scroll zoom, and a scrolling mode setting.
- Command shortcuts on macOS and Ctrl shortcuts on Windows/Linux, logical
  AZERTY/QWERTY letters, text-field protection, new/open and view shortcuts.
- Scrollable Properties and tool panels, plus protection against canvas input
  while a confirmation or settings dialog is open.
- Command-line and drag-and-drop project opening with unsaved-change protection.
- Rust style review, explicit imports, strict Clippy checks and multi-platform CI.

Downloads include Windows x64, Linux x64, macOS Apple Silicon and macOS Intel,
plus SHA-256 checksums. The Linux build targets Ubuntu 24.04-compatible systems.
Windows binaries are unsigned; macOS apps use an ad-hoc signature and are not
Apple notarized. Native pinch availability depends on the OS/windowing backend;
modified scrolling and keyboard/on-screen controls provide alternatives.

Automated build and regression checks run on each platform. Physical testing of
every mouse, trackpad, keyboard layout and Linux desktop is not claimed.
