import assert from "node:assert/strict";
import { test } from "node:test";
import { formatScaleLabel, formatTemperature, formatWeatherDay } from "../../public/js/format.js";

test("formatTemperature keeps compact integer labels", () => {
  assert.equal(formatTemperature(26), "26°C");
  assert.equal(formatTemperature(26.5), "26.5°C");
  assert.equal(formatTemperature(null), "--");
});

test("formatScaleLabel does not add useless decimals", () => {
  assert.equal(formatScaleLabel(21), "21");
  assert.equal(formatScaleLabel(21.5), "21.5");
});

test("formatWeatherDay formats localized weekday labels", () => {
  assert.equal(formatWeatherDay("2026-04-23", "en"), "Thu");
  assert.equal(formatWeatherDay("2026-04-23", "ru"), "Чт");
});
