import assert from "node:assert/strict";
import { test } from "node:test";
import {
  iconNameForTheme,
  spriteSize,
  spriteUrl,
  versionedUrl,
  weatherIconUrl,
} from "../../public/js/assets.js";

test("theme button resolves to theme-specific icons", () => {
  assert.equal(iconNameForTheme("theme_button", "light"), "theme_button_sun");
  assert.equal(iconNameForTheme("theme_button", "dark"), "theme_button_moon");
});

test("sprite sizes use resolved theme icon dimensions", () => {
  assert.deepEqual(spriteSize("theme_button", "dark"), [48, 48]);
  assert.deepEqual(spriteSize("unknown_icon", "light"), [32, 32]);
});

test("spriteUrl adds cache version to themed assets", () => {
  assert.match(spriteUrl("plus", "dark", "v1"), /^\/assets\/dark\/icons\/plus\.png\?v=v1$/);
});

test("weatherIconUrl preserves cached icon paths", () => {
  assert.equal(weatherIconUrl("/weather-icons/cloud.png", "light", "v2"), "/weather-icons/cloud.png?v=v2");
});

test("versionedUrl appends with ampersand when URL already has query", () => {
  assert.equal(versionedUrl("/asset.png?x=1", "v3"), "/asset.png?x=1&v=v3");
});
