# MelCloud ATA Workspace

Small Rust workspace for one classic MELCloud ATA device.

## Packages

- `melcloud-core`: low-level MELCloud transport, discovery, config payloads, and remote preset slots
- `melcloud-cli`: operator-facing CLI on top of the core API
- `melcloud-site`: HTML site + Rust server that calls `melcloud-cli` as a subprocess

## Scope

- classic MELCloud only: `https://app.melcloud.com/`
- one ATA device only
- no polling or daemon behavior
- Telegram bot remains out of scope in this workspace phase

## Build

```powershell
cargo build --release --workspace
```

Release binaries:

```text
target\release\melcloud-cli.exe
target\release\melcloud-site.exe
```

Portable runtime package:

```powershell
.\build.ps1
```

Ubuntu VPS installation:

- [Setup.md](Setup.md)

This creates `.\build\` with:

- `bin\melcloud-cli.exe`
- `bin\melcloud-site.exe`
- `melcloud-site\public\`
- `melcloud-site\site-assets\`
- `melcloud-site\melcloud-site.yaml`
- `melcloud-cli\presets\`
- `melcloud-cli\state\`
- `melcloud-site\state\`
- `melcloud-site\cache\`
- `.env`, if it exists
- `run-site.cmd`

Use `.\build.ps1 -NoEnv` to create a package without credentials. Use `.\build.ps1 -NoRuntimeState` to skip copying current runtime state.

## Credentials and language

Lookup order for login:

1. CLI flags
2. `MELCLOUD_LOGIN` / `MELCLOUD_PASSWORD`
3. `.env` in repo root with:

```text
login=...
password=...
language=en
```

Values may be unquoted or wrapped in single/double quotes, for example `password="..."`.

`language` is used by:

- `melcloud-cli` login payload (`en`, `ru`, or numeric MELCloud language id)

Site UI localization is configured separately in `.\melcloud-site\melcloud-site.yaml`.

## Runtime layout

- local CLI presets: `.\melcloud-cli\presets\*.yaml`
- fixed site presets: `.\melcloud-cli\presets\site-heat.yaml`, `site-fan.yaml`, `site-cool.yaml`, `site-dry.yaml`
- CLI session cache: `.\melcloud-cli\state\session.json`
- CLI device profile: `.\melcloud-cli\state\device.yaml`
- remote preset backups: `.\melcloud-cli\state\remote-preset-backups\slot-<n>.json`
- site selected preset state: `.\melcloud-site\state\site-state.json`
- site weather icon cache: `.\melcloud-site\cache\weather-icons\`
- site config: `.\melcloud-site\melcloud-site.yaml`

## CLI contract

- login: `Login/ClientLogin3` with fallback to `Login/ClientLogin`
- discovery: `User/ListDevices`
- read: `Device/Get`
- config write: `Device/SetAta`
- remote preset save: `Device/SetAtaPreset`

Writable config fields:

- `power`
- `mode`
- `target_temperature`
- `fan_speed`
- `vane_horizontal`
- `vane_vertical`

## Site contract

- `melcloud-site` does not talk to MELCloud directly
- live reads and writes go through `melcloud-cli` subprocess calls
- site presets are local YAML files, not MELCloud remote preset slots
- active preset is global server-side state in `site-state.json`
- config edits are debounced in the browser and flushed after idle time from `melcloud-site\melcloud-site.yaml`
- preset selection and follow-up edits made before the debounce fires are flushed as one desired config write
- site CLI calls have a server-side timeout, and writes are confirmed by one read-back before autosaving the active site preset
- after a successful site write, the resulting live state is written back into the active site preset
- site state and fixed site preset writes keep `.bak` files for recovery from interrupted writes
- weather cards render with local fallback icons first; remote weather icons are cached in the background

## Run

CLI:

```powershell
.\build\bin\melcloud-cli.exe status --json
.\build\bin\melcloud-cli.exe remote-preset list --json
```

Site:

```powershell
.\build\run-site.cmd
```

Development site:

```powershell
cargo run -p melcloud-site
```

Full site subprocess tests:

```powershell
cargo test -p melcloud-site -- --ignored
```

Default bind:

```text
0.0.0.0:8787
```

## Package notes

- [melcloud-core/README.md](melcloud-core/README.md)
- [melcloud-cli/README.md](melcloud-cli/README.md)
- [melcloud-site/README.md](melcloud-site/README.md)
