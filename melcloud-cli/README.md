# melcloud-cli

Agent-oriented notes for the Windows-first CLI that wraps `melcloud-core`.

## Scope

- one classic MELCloud ATA device
- high-level UX on top of the absolute core API
- local YAML presets
- server-side MELCloud preset slots
- subprocess backend for `melcloud-site`

## Responsibilities

The CLI may do:

- path resolution
- env loading
- relative temperature parsing
- preview formatting
- YAML preset parsing and validation
- rollback file management for remote preset slot overwrite and restore
- convergence polling after writes

The CLI must not:

- redefine the MELCloud wire contract
- invent alternate write payloads when the official one is known
- hide partial rollback or verification failures

## Runtime layout

- local presets: `.\melcloud-cli\presets\*.yaml`
- fixed site presets: `.\melcloud-cli\presets\site-*.yaml`
- session cache: `.\melcloud-cli\state\session.json`
- device profile: `.\melcloud-cli\state\device.yaml`
- remote preset backups: `.\melcloud-cli\state\remote-preset-backups\slot-<n>.json`

## Credentials and language

- login/password come from flags, environment, or repo-root `.env`
- `--language` accepts `en`, `ru`, or a numeric MELCloud language id
- `.env` may include `language=en|ru|<id>`
- `.env` values may be unquoted or wrapped in single/double quotes
- site UI only uses `en` or `ru`, but the CLI keeps numeric language-id passthrough

## Command surface

- `auth test`
- `devices list`
- `devices sync`
- `status`
- `weather`
- `set --power/--mode/--target-temperature/--fan-speed/--vane-horizontal/--vane-vertical [--preview] [--verify]`
- `preset list|show|init|capture|preview|set-field|apply`
- `remote-preset list|show|preview|export|apply|save|delete`

## CLI semantics

### Relative temperature

Relative temperature input exists only in the CLI:

- `--target-temperature +1`
- `--target-temperature -0.5`

The CLI reads the current config, resolves the relative delta into an absolute target, and only then calls the core API.

### Local YAML presets

Local presets are UX helpers, not MELCloud-native objects.

Rules:

- all state keys are optional
- missing keys are not sent directly
- merge with live state happens only at preview or apply time
- unknown keys are rejected

### Remote MELCloud presets

Remote MELCloud presets are treated as fixed slots `#1/#2/#3`.

Rules:

- `remote-preset list/show` read directly from `User/ListDevices`
- `remote-preset preview` shows the diff between live config and the selected remote preset
- `remote-preset export` writes a local YAML preset without touching the device
- `remote-preset apply` uses the remote preset as a full config snapshot and applies it through the config path
- `remote-preset save` stores a rollback backup before overwriting a slot
- `remote-preset delete` restores the previous slot contents from the local rollback backup

## Verification behavior

- `set --verify`, `preset apply`, `remote-preset apply`, `remote-preset save`, and `remote-preset delete` wait for convergence
- one immediate read-back is not treated as sufficient proof of success

## Non-goals

- daemon mode
- background polling
- GUI workflows
- multi-device selection UX
