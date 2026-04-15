import assert from "node:assert/strict";
import { test } from "node:test";
import { clampTemperature } from "../../public/js/domain.js";
import { sliderTemperature } from "../../public/js/render.js";

test("sliderTemperature derives its range from DOM geometry", () => {
  const elements = {
    tempSliderTicks: {
      getBoundingClientRect: () => ({ top: 100, height: 300 }),
    },
  };
  const snapshot = {
    active_preset_id: "site-cool",
    live_status: { operation_mode: "cool" },
    capabilities: {
      min_temp_cool_dry: 16,
      max_temp_cool_dry: 31,
      temperature_step: 0.5,
    },
  };
  const bounds = { min: 16, max: 31 };

  assert.equal(sliderTemperature(elements, snapshot, 100, clampTemperature, bounds), 31);
  assert.equal(sliderTemperature(elements, snapshot, 250, clampTemperature, bounds), 23.5);
  assert.equal(sliderTemperature(elements, snapshot, 400, clampTemperature, bounds), 16);
});
