import assert from "node:assert/strict";
import { test } from "node:test";
import {
  activeMode,
  applyOptimisticPatch,
  applyOptimisticPreset,
  clampTemperature,
  currentTemperatureBounds,
  mergePatch,
} from "../../public/js/domain.js";

test("mergePatch overwrites only provided fields", () => {
  assert.deepEqual(mergePatch({ power: true, fan_speed: "2" }, { fan_speed: "4" }), {
    power: true,
    fan_speed: "4",
  });
});

test("temperature bounds follow active preset mode", () => {
  const snapshot = sampleSnapshot({
    active_preset_id: "site-heat",
    capabilities: {
      min_temp_heat: 10,
      max_temp_heat: 31,
      min_temp_cool_dry: 16,
      max_temp_cool_dry: 30,
      temperature_step: 0.5,
    },
  });
  assert.equal(activeMode(snapshot), "heat");
  assert.deepEqual(currentTemperatureBounds(snapshot), { min: 10, max: 31 });
});

test("clampTemperature snaps to the configured increment and bounds", () => {
  const snapshot = sampleSnapshot({
    capabilities: {
      min_temp_cool_dry: 16,
      max_temp_cool_dry: 31,
      temperature_step: 0.5,
    },
  });
  assert.equal(clampTemperature(22.26, snapshot), 22.5);
  assert.equal(clampTemperature(7, snapshot), 16);
  assert.equal(clampTemperature(40, snapshot), 31);
});

test("applyOptimisticPatch updates live status and numeric codes", () => {
  const snapshot = sampleSnapshot();
  const next = applyOptimisticPatch(snapshot, {
    power: false,
    mode: "fan_only",
    target_temperature: 24.5,
    fan_speed: "3",
    vane_horizontal: "5",
    vane_vertical: "auto",
  });
  assert.notEqual(next, snapshot);
  assert.deepEqual(next.live_status, {
    ...snapshot.live_status,
    power: false,
    operation_mode: "fan_only",
    operation_mode_code: 7,
    target_temperature: 24.5,
    fan_speed: "3",
    fan_speed_code: 3,
    vane_horizontal: "5",
    vane_horizontal_code: 5,
    vane_vertical: "auto",
    vane_vertical_code: null,
  });
});

test("applyOptimisticPreset uses stored preset config when available", () => {
  const snapshot = sampleSnapshot({
    presets: [
      { id: "site-cool", config: { mode: "cool", target_temperature: 21, fan_speed: "2" } },
    ],
  });
  const next = applyOptimisticPreset(snapshot, "site-cool");
  assert.equal(next.active_preset_id, "site-cool");
  assert.equal(next.live_status.operation_mode, "cool");
  assert.equal(next.live_status.target_temperature, 21);
  assert.equal(next.live_status.fan_speed, "2");
});

function sampleSnapshot(overrides = {}) {
  return {
    active_preset_id: "site-cool",
    live_status: {
      power: true,
      operation_mode: "cool",
      operation_mode_code: 3,
      room_temperature: 26,
      target_temperature: 25,
      fan_speed: "1",
      fan_speed_code: 1,
      vane_horizontal: "auto",
      vane_horizontal_code: null,
      vane_vertical: "1",
      vane_vertical_code: 1,
    },
    capabilities: {
      fan_speeds: [1, 2, 3, 4, 5],
      supports_fan_auto: true,
      min_temp_auto: 16,
      max_temp_auto: 31,
      min_temp_cool_dry: 16,
      max_temp_cool_dry: 31,
      min_temp_heat: 10,
      max_temp_heat: 31,
      temperature_step: 0.5,
      ...(overrides.capabilities || {}),
    },
    presets: [],
    ...overrides,
  };
}
