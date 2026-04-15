# melcloud-core

Agent-oriented notes for the low-level classic MELCloud integration.

## Scope

- classic MELCloud only
- one ATA device only
- transport, discovery, config mapping, and remote preset slot mapping
- no CLI-only convenience semantics

## Domain model

### Config

`config` is the absolute ATA state sent in one `Device/SetAta` request.

Supported writable fields:

- `power`
- `mode`
- `target_temperature`
- `fan_speed`
- `vane_horizontal`
- `vane_vertical`

Core stays absolute. Relative operations such as `+1` degrees belong in the CLI layer only.

### Remote preset

`remote preset` is a MELCloud server-side snapshot stored in a fixed slot.

Rules:

- treat classic MELCloud presets as slot-based: `#1/#2/#3`
- read source: `User/ListDevices -> Structure -> Devices[*] -> Presets`
- save source: `POST /Device/SetAtaPreset`
- apply path: convert the preset into an absolute config patch and send it through `Device/SetAta`

### Discovery

- `User/ListDevices` is the source of truth for bootstrap and remote preset discovery
- the full ATA device node must be preserved
- `BoundDevice` is a derived control view, not the primary discovery model

## Network contract

### Login

- primary: `Login/ClientLogin3`
- fallback: `Login/ClientLogin`
- cached session is reused until expiry or server rejection
- login payload language id is caller-supplied; `0` remains the default

### Reads

- device state: `Device/Get`
- weather comes from `WeatherObservations` inside the same payload

### Writes

- config write: `Device/SetAta`
- remote preset save: `Device/SetAtaPreset`

Confirmed remote preset save payload shape:

```json
{
  "DeviceId": 12066563,
  "Number": "3",
  "NumberDescription": "PresetName",
  "PresetRequest": {
    "Power": true,
    "SetTemperature": 26,
    "OperationMode": 7,
    "VaneHorizontal": 5,
    "VaneVertical": 1,
    "FanSpeed": 3
  }
}
```

## Expected public API

Keep the stable surface close to these operations:

- `get_device_status()`
- `get_current_config()`
- `prepare_config_command(current, patch)`
- `send_config_command(device, prepared)`
- `apply_config_patch(device, patch)`
- `list_device_nodes()`
- `list_remote_presets()`
- `save_remote_preset(request)`
- `apply_remote_preset(device, preset)`

`MelcloudConfig` also carries:

- credentials
- session cache path
- login language id
- request timeout

## Reliability rules

- clear stale session cache before re-login on `401/403`
- allow one retry for transient transport failures
- verify write requests by polling in the caller, not by assuming one immediate read is enough
- let the caller choose the session cache path; do not hardcode a platform-specific app-data location into the public contract

## Non-goals

- multi-device support
- non-ATA device types
- timers, scenes, or schedules as a dedicated feature set
- background polling
- Telegram bot orchestration
