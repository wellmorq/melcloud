# melcloud-site

Agent-oriented notes for the HTML site and its Rust server wrapper.

## Scope

- one-page HTML/CSS/JS UI
- one classic MELCloud ATA device
- no direct MELCloud protocol handling inside the site package
- no frontend framework

## Responsibilities

The site package is responsible for:

- serving static HTML/CSS/JS
- calling `melcloud-cli` as a subprocess for live reads and writes
- maintaining fixed local site presets:
  - `site-heat`
  - `site-fan`
  - `site-cool`
  - `site-dry`
- keeping the globally selected preset in `.\melcloud-site\state\site-state.json`
- translating UI strings for `en` and `ru`

The site package must not:

- reimplement MELCloud transport or session logic
- overwrite MELCloud remote preset slots for normal UI flows
- poll continuously in the background

## Runtime layout

- public files: `melcloud-site/public`
- runtime assets: `melcloud-site/site-assets`
- local fonts: `melcloud-site/site-assets/fonts`
- source/reference assets: `melcloud-site/reference`
- site config: `.\melcloud-site\melcloud-site.yaml`
- CLI path: packaged runtime `bin/melcloud-cli(.exe)` by default, or `.env` / env override
- preset directory passed to CLI subprocesses: `.\melcloud-cli\presets`
- CLI state files passed to subprocesses: `.\melcloud-cli\state`
- fixed site presets: `.\melcloud-cli\presets\site-*.yaml`
- selected preset state: `.\melcloud-site\state\site-state.json`
- weather icon cache: `.\melcloud-site\cache\weather-icons\`

The packaged runtime created by root `.\build.ps1` keeps the same relative layout inside `.\build\`.

`melcloud-site.yaml` currently controls:

- `bind_addr`
- `ui_language`
- `commit_debounce_ms`
- `cli_timeout_ms`
- `weather_icon_timeout_ms`

## HTTP API

- `GET /api/state`
- `POST /api/refresh`
- `POST /api/presets/{id}/apply`
- `PATCH /api/presets/{id}/config`

`{id}` is one of:

- `site-heat`
- `site-fan`
- `site-cool`
- `site-dry`

## UI rules

- one shared component set for both vertical and horizontal layouts
- layout changes only through responsive CSS
- theme is browser-local state
- active preset is server-side state
- config edits update browser-local state immediately
- config writes are debounced and flushed only after `commit_debounce_ms` of inactivity
- preset selection plus any follow-up control edits before debounce are merged and synced as one desired config state
- slider drag updates locally and commits into the debounced config state on release
- wheel-based temperature changes are captured only inside the slider hit-area

## CLI orchestration rules

- reads use `status --json` and `devices list --json`
- normal UI writes use `set ... --json --verify=false`, including preset selection
- direct `POST /api/presets/{id}/apply` remains available, but the browser UI uses `PATCH /api/presets/{id}/config`
- after a config mutation, the site runs one `status --json` read-back
- the active preset YAML in `melcloud-cli\presets` is updated only after the read-back confirms the requested write
- preset/state writes keep `.bak` files and read from backup if the primary file is missing or corrupt
- weather icon downloads must not block `/api/state`; missing remote icons fall back immediately and cache in the background

## Tests

Normal test pass:

```powershell
cargo test --workspace
npm --prefix melcloud-site run test:js
```

Process-spawning runner tests are ignored by default and should be run explicitly where subprocess execution is allowed:

```powershell
cargo test -p melcloud-site -- --ignored
```

## Non-goals

- authentication UI
- MELCloud remote preset management from the site
- websocket push updates
- generic multi-user state separation
