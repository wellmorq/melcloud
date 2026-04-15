import { applySprite, applyWeatherIcon } from "./assets.js";
import { currentTemperatureBounds, presetDefs, vanePatch } from "./domain.js";
import { formatTemperature, formatWeatherDay, formatScaleLabel } from "./format.js";

export function readElements(documentRef = document) {
  const byId = (id) => requireElement(documentRef.getElementById(id), `#${id}`);
  const appShell = byId("app-shell");
  return {
    document: documentRef,
    appShell,
    banner: byId("banner"),
    deviceName: byId("device-name"),
    powerToggle: byId("power-toggle"),
    powerLabel: requireElement(appShell.querySelector(".power-toggle__label"), ".power-toggle__label"),
    syncStatus: byId("sync-status"),
    syncStatusLabel: byId("sync-status-label"),
    themeToggle: byId("theme-toggle"),
    themeIcon: byId("theme-icon"),
    homeIcon: byId("home-icon"),
    minusIcon: byId("minus-icon"),
    plusIcon: byId("plus-icon"),
    fanAutoIcon: byId("fan-auto-icon"),
    vaneHorizontalAutoIcon: byId("vane-horizontal-auto-icon"),
    vaneVerticalAutoIcon: byId("vane-vertical-auto-icon"),
    roomTemperature: byId("room-temperature"),
    targetTemperature: byId("target-temperature"),
    tempMinus: byId("temp-minus"),
    tempPlus: byId("temp-plus"),
    fanButtons: byId("fan-buttons"),
    fanAuto: byId("fan-auto"),
    presetButtons: byId("preset-buttons"),
    weatherCards: byId("weather-cards"),
    vaneHorizontalButtons: byId("vane-horizontal-buttons"),
    vaneVerticalButtons: byId("vane-vertical-buttons"),
    vaneHorizontalAuto: byId("vane-horizontal-auto"),
    vaneVerticalAuto: byId("vane-vertical-auto"),
    tempSlider: byId("temp-slider"),
    tempSliderTicks: requireElement(appShell.querySelector(".temp-slider__ticks"), ".temp-slider__ticks"),
    tempSliderThumb: byId("temp-slider-thumb"),
    tempMinLabel: byId("temp-min-label"),
    tempUpperLabel: byId("temp-upper-label"),
    tempLowerLabel: byId("temp-lower-label"),
    tempMaxLabel: byId("temp-max-label"),
    i18nLabels: [...documentRef.querySelectorAll("[data-i18n]")],
  };
}

export function createRenderer(elements, handlers) {
  const cache = {
    fanSignature: "",
    fanButtons: [],
    presetButtons: new Map(),
    weatherSignature: "",
    weatherTiles: [],
    vaneButtons: { horizontal: [], vertical: [] },
  };

  return {
    render(snapshot, context) {
      const { theme, targetTemperature, translate } = context;
      elements.document.documentElement.lang = snapshot.language;
      renderTranslations(elements, snapshot.language, translate);
      renderHeader(elements, snapshot);
      renderStaticSprites(elements, theme);
      renderFanButtons(elements, cache, snapshot, theme, handlers);
      renderPresetButtons(elements, cache, snapshot, theme, translate, handlers);
      renderWeatherCards(elements, cache, snapshot, theme, translate);
      renderVaneButtons(elements, cache, "horizontal", snapshot.live_status.vane_horizontal, handlers);
      renderVaneButtons(elements, cache, "vertical", snapshot.live_status.vane_vertical, handlers);
      elements.vaneHorizontalAuto.classList.toggle("is-active", snapshot.live_status.vane_horizontal === "auto");
      elements.vaneVerticalAuto.classList.toggle("is-active", snapshot.live_status.vane_vertical === "auto");
      elements.fanAuto.hidden = !snapshot.capabilities.supports_fan_auto;
      elements.roomTemperature.textContent = formatTemperature(snapshot.live_status.room_temperature);
      elements.targetTemperature.textContent = formatTemperature(targetTemperature);
      updateSlider(elements, snapshot, targetTemperature, context.temperatureBounds);
    },
  };
}

export function showBanner(elements, message, isError = false) {
  elements.banner.hidden = false;
  elements.banner.textContent = message;
  elements.banner.style.background = isError
    ? "rgba(255, 96, 96, 0.12)"
    : "rgba(45, 140, 255, 0.12)";
}

export function clearBanner(elements) {
  elements.banner.hidden = true;
}

export function updateSyncStatus(elements, status, label) {
  elements.syncStatus.dataset.state = status;
  elements.syncStatus.setAttribute("aria-label", label);
  elements.syncStatus.title = label;
  elements.syncStatusLabel.textContent = label;
}

export function sliderTemperature(elements, snapshot, clientY, clampTemperature, bounds) {
  const rect = elements.tempSliderTicks.getBoundingClientRect();
  const ratio = 1 - Math.min(Math.max((clientY - rect.top) / rect.height, 0), 1);
  const raw = bounds.min + ratio * (bounds.max - bounds.min);
  return clampTemperature(raw, snapshot, bounds);
}

function updateSlider(elements, snapshot, current, bounds = null) {
  const resolvedBounds = bounds || currentTemperatureBounds(snapshot);
  const clamped = Math.min(resolvedBounds.max, Math.max(resolvedBounds.min, current));
  const ratio =
    resolvedBounds.max === resolvedBounds.min
      ? 0
      : (clamped - resolvedBounds.min) / (resolvedBounds.max - resolvedBounds.min);
  const geometry = sliderGeometry(elements);
  elements.tempSliderThumb.style.top = `${geometry.top + (1 - ratio) * geometry.height}px`;
  const step = (resolvedBounds.max - resolvedBounds.min) / 3;
  elements.tempMinLabel.textContent = formatScaleLabel(resolvedBounds.min);
  elements.tempLowerLabel.textContent = formatScaleLabel(resolvedBounds.min + step);
  elements.tempUpperLabel.textContent = formatScaleLabel(resolvedBounds.min + step * 2);
  elements.tempMaxLabel.textContent = formatScaleLabel(resolvedBounds.max);
}

function sliderGeometry(elements) {
  const sliderRect = elements.tempSlider.getBoundingClientRect();
  const ticksRect = elements.tempSliderTicks.getBoundingClientRect();
  return {
    top: ticksRect.top - sliderRect.top,
    height: ticksRect.height,
  };
}

function renderTranslations(elements, language, translate) {
  elements.i18nLabels.forEach((element) => {
    element.textContent = translate(element.dataset.i18n, language);
  });
}

function renderHeader(elements, snapshot) {
  elements.deviceName.textContent = snapshot.device.name;
  elements.powerToggle.classList.toggle("is-off", !snapshot.live_status.power);
  elements.powerLabel.textContent = snapshot.live_status.power ? "ON" : "OFF";
}

function renderStaticSprites(elements, theme) {
  applySprite(elements.themeIcon, "theme_button", theme);
  applySprite(elements.homeIcon, "home", theme);
  applySprite(elements.minusIcon, "minus", theme);
  applySprite(elements.plusIcon, "plus", theme);
  applySprite(elements.fanAutoIcon, "auto_refresh", theme);
  applySprite(elements.vaneHorizontalAutoIcon, "vane_auto_horizontal", theme);
  applySprite(elements.vaneVerticalAutoIcon, "vane_auto_vertical", theme);
}

function renderFanButtons(elements, cache, snapshot, theme, handlers) {
  const speeds = snapshot.capabilities.fan_speeds.map(String);
  const signature = speeds.join("|");
  if (cache.fanSignature !== signature) {
    cache.fanSignature = signature;
    cache.fanButtons = speeds.map((speed) => createFanButton(elements.document, speed, handlers));
    elements.fanButtons.replaceChildren(...cache.fanButtons.map((entry) => entry.button));
  }

  const current = snapshot.live_status.fan_speed ?? "";
  cache.fanButtons.forEach(({ speed, button, icon }) => {
    const isActive = current === speed;
    button.classList.toggle("is-active", isActive);
    applySprite(icon, isActive ? "fan_blades_small_active" : "fan_blades_small_idle", theme);
  });
  elements.fanAuto.classList.toggle("is-active", current === "auto");
}

function createFanButton(documentRef, speed, handlers) {
  const button = documentRef.createElement("button");
  button.className = "fan-button";
  button.type = "button";
  const speedLabel = documentRef.createElement("span");
  speedLabel.className = "fan-button__speed";
  speedLabel.textContent = speed;
  const icon = documentRef.createElement("span");
  icon.className = "sprite fan-button__icon";
  icon.setAttribute("aria-hidden", "true");
  button.append(speedLabel, icon);
  button.addEventListener("click", () => handlers.mutateActive({ fan_speed: speed }));
  return { speed, button, icon };
}

function renderPresetButtons(elements, cache, snapshot, theme, translate, handlers) {
  if (cache.presetButtons.size === 0) {
    const buttons = presetDefs.map((preset) => createPresetButton(elements.document, preset, handlers));
    buttons.forEach((entry) => cache.presetButtons.set(entry.preset.id, entry));
    elements.presetButtons.replaceChildren(...buttons.map((entry) => entry.button));
  }

  presetDefs.forEach((preset) => {
    const entry = cache.presetButtons.get(preset.id);
    const label = translate(preset.label, snapshot.language);
    entry.button.title = label;
    entry.button.setAttribute("aria-label", label);
    entry.button.classList.toggle("is-active", snapshot.active_preset_id === preset.id);
    applySprite(entry.icon, preset.icon, theme);
  });
}

function createPresetButton(documentRef, preset, handlers) {
  const button = documentRef.createElement("button");
  button.className = `preset-button preset-button--${preset.id}`;
  button.type = "button";
  const icon = documentRef.createElement("span");
  icon.className = "sprite preset-button__icon";
  icon.setAttribute("aria-hidden", "true");
  button.append(icon);
  button.addEventListener("click", () => handlers.applyPreset(preset.id));
  return { preset, button, icon };
}

function renderWeatherCards(elements, cache, snapshot, theme, translate) {
  const signature = snapshot.weather_cards.map((card) => card.slot).join("|");
  if (cache.weatherSignature !== signature) {
    cache.weatherSignature = signature;
    cache.weatherTiles = snapshot.weather_cards.map(() => createWeatherTile(elements.document));
    elements.weatherCards.replaceChildren(...cache.weatherTiles.map((entry) => entry.tile));
  }

  snapshot.weather_cards.forEach((card, index) => {
    const entry = cache.weatherTiles[index];
    entry.tile.classList.toggle("is-placeholder", card.placeholder);
    entry.day.textContent = card.placeholder ? "" : formatWeatherDay(card.date, snapshot.language);
    entry.period.textContent = translate(card.period_key, snapshot.language);
    entry.temp.textContent = card.temperature == null ? "" : formatTemperature(card.temperature);
    if (!card.placeholder) {
      applyWeatherIcon(entry.icon, card.icon, theme);
    }
  });
}

function createWeatherTile(documentRef) {
  const tile = documentRef.createElement("div");
  tile.className = "weather-tile";

  const header = documentRef.createElement("div");
  header.className = "weather-tile__header";
  const day = documentRef.createElement("span");
  day.className = "weather-tile__day";
  const period = documentRef.createElement("span");
  period.className = "weather-tile__period";
  header.append(day, period);

  const icon = documentRef.createElement("span");
  icon.className = "sprite weather-tile__icon";
  icon.setAttribute("aria-hidden", "true");

  const temp = documentRef.createElement("div");
  temp.className = "weather-tile__temp";

  tile.append(header, icon, temp);
  return { tile, day, period, icon, temp };
}

function renderVaneButtons(elements, cache, axis, currentValue, handlers) {
  const target = axis === "horizontal" ? elements.vaneHorizontalButtons : elements.vaneVerticalButtons;
  if (cache.vaneButtons[axis].length === 0) {
    cache.vaneButtons[axis] = [1, 2, 3, 4, 5].map((step) =>
      createVaneButton(elements.document, axis, step, handlers),
    );
    target.replaceChildren(...cache.vaneButtons[axis].map((entry) => entry.button));
  }

  cache.vaneButtons[axis].forEach(({ step, button }) => {
    button.classList.toggle("is-active", currentValue === String(step));
  });
}

function createVaneButton(documentRef, axis, step, handlers) {
  const button = documentRef.createElement("button");
  button.className = "vane-button";
  button.type = "button";
  const bar = documentRef.createElement("span");
  bar.className = "vane-button__bar";
  button.append(bar);
  button.addEventListener("click", () => handlers.mutateActive(vanePatch(axis, step)));
  return { step: String(step), button };
}

function requireElement(element, selector) {
  if (!element) {
    throw new Error(`Missing required UI element ${selector}.`);
  }
  return element;
}
