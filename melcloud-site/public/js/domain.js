export const presetDefs = [
  { id: "site-heat", icon: "preset_heat", mode: "heat", label: "presetHeat" },
  { id: "site-fan", icon: "preset_fan", mode: "fan_only", label: "presetFan" },
  { id: "site-cool", icon: "preset_cool", mode: "cool", label: "presetCool" },
  { id: "site-dry", icon: "preset_dry", mode: "dry", label: "presetDry" },
];

export function mergePatch(basePatch, nextPatch) {
  return {
    ...(basePatch || {}),
    ...nextPatch,
  };
}

export function activeMode(snapshot) {
  const preset = presetDefs.find((item) => item.id === snapshot?.active_preset_id);
  return preset?.mode ?? snapshot?.live_status?.operation_mode ?? "cool";
}

export function currentTemperatureBounds(snapshot, mode = activeMode(snapshot)) {
  const capabilities = snapshot?.capabilities;
  if (!capabilities) return { min: 16, max: 31 };
  if (mode === "heat") {
    return { min: capabilities.min_temp_heat ?? 10, max: capabilities.max_temp_heat ?? 31 };
  }
  if (mode === "auto") {
    return { min: capabilities.min_temp_auto ?? 16, max: capabilities.max_temp_auto ?? 31 };
  }
  return { min: capabilities.min_temp_cool_dry ?? 16, max: capabilities.max_temp_cool_dry ?? 31 };
}

export function temperatureStep(snapshot) {
  return snapshot?.capabilities?.temperature_step || 0.5;
}

export function clampTemperature(value, snapshot, bounds = currentTemperatureBounds(snapshot)) {
  const step = temperatureStep(snapshot);
  const rounded = Math.round(value / step) * step;
  return Math.min(bounds.max, Math.max(bounds.min, Number(rounded.toFixed(1))));
}

export function currentTargetTemperature(snapshot, tempDraft = null) {
  return tempDraft ?? snapshot?.live_status?.target_temperature ?? currentTemperatureBounds(snapshot).min;
}

export function applyOptimisticPreset(snapshot, presetId) {
  if (!snapshot) return snapshot;
  const next = structuredClone(snapshot);
  next.active_preset_id = presetId;

  const preset = next.presets?.find((item) => item.id === presetId);
  if (preset?.config) {
    return applyOptimisticPatch(next, preset.config);
  }

  const presetDef = presetDefs.find((item) => item.id === presetId);
  if (presetDef) {
    next.live_status.operation_mode = presetDef.mode;
    next.live_status.operation_mode_code = modeCodeFromValue(presetDef.mode);
  }
  return next;
}

export function applyOptimisticPatch(snapshot, patch) {
  if (!snapshot) return snapshot;
  const next = structuredClone(snapshot);
  const live = next.live_status;

  if (typeof patch.power === "boolean") {
    live.power = patch.power;
  }
  if (patch.mode) {
    live.operation_mode = patch.mode;
    live.operation_mode_code = modeCodeFromValue(patch.mode);
  }
  if (typeof patch.target_temperature === "number") {
    live.target_temperature = patch.target_temperature;
  }
  if (patch.fan_speed) {
    live.fan_speed = patch.fan_speed;
    live.fan_speed_code = numericCode(patch.fan_speed);
  }
  if (patch.vane_horizontal) {
    live.vane_horizontal = patch.vane_horizontal;
    live.vane_horizontal_code = numericCode(patch.vane_horizontal);
  }
  if (patch.vane_vertical) {
    live.vane_vertical = patch.vane_vertical;
    live.vane_vertical_code = numericCode(patch.vane_vertical);
  }
  return next;
}

export function modeCodeFromValue(mode) {
  return {
    heat: 1,
    dry: 2,
    cool: 3,
    auto: 8,
    fan_only: 7,
  }[mode] ?? null;
}

export function numericCode(value) {
  return /^\d+$/.test(value) ? Number(value) : null;
}

export function vanePatch(axis, step) {
  return axis === "horizontal" ? { vane_horizontal: String(step) } : { vane_vertical: String(step) };
}
