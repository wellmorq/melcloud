# Changelog

## v1.0.0

- Added a Rust CLI for one classic MELCloud ATA device.
- Added local YAML presets owned by the CLI runtime.
- Added a local HTML site that delegates live MELCloud reads and writes to the CLI.
- Fixed site debounced sync so preset selection plus immediate control edits are sent as one desired config write.
- Added packaged Windows/Linux runtime layout with binaries under `build/bin`.
- Added Ubuntu VPS deployment instructions in `Setup.md`.
