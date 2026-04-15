import { assetVersion } from "./build-version.js";

export const iconSizes = {
  theme_button_sun: [48, 48],
  theme_button_moon: [48, 48],
  home: [28, 28],
  minus: [31, 31],
  plus: [31, 31],
  auto_refresh: [46, 46],
  vane_auto_horizontal: [36, 36],
  vane_auto_vertical: [36, 36],
  fan_blades_small_idle: [30, 30],
  fan_blades_small_active: [30, 30],
  preset_heat: [48, 48],
  preset_fan: [48, 48],
  preset_cool: [48, 48],
  preset_dry: [48, 48],
  weather_sun: [52, 52],
  weather_moon: [52, 52],
  weather_cloud: [52, 52],
};

export function iconNameForTheme(iconName, theme) {
  if (iconName === "theme_button") {
    return theme === "dark" ? "theme_button_moon" : "theme_button_sun";
  }
  return iconName;
}

export function versionedUrl(url, version = assetVersion) {
  const separator = url.includes("?") ? "&" : "?";
  return `${url}${separator}v=${encodeURIComponent(version)}`;
}

export function spriteUrl(iconName, theme, version = assetVersion) {
  const resolved = iconNameForTheme(iconName, theme);
  return versionedUrl(`/assets/${theme}/icons/${resolved}.png`, version);
}

export function spriteSize(iconName, theme) {
  const resolved = iconNameForTheme(iconName, theme);
  return iconSizes[resolved] || [32, 32];
}

export function weatherIconUrl(iconName, theme, version = assetVersion) {
  if (iconName.startsWith("/")) {
    return versionedUrl(iconName, version);
  }
  const resolved = iconNameForTheme(iconName, theme);
  return versionedUrl(`/assets/${theme}/icons/${resolved}.png`, version);
}

export function applySprite(element, iconName, theme) {
  if (!element) return;
  const [width, height] = spriteSize(iconName, theme);
  element.style.width = `${width}px`;
  element.style.height = `${height}px`;
  element.style.backgroundImage = `url("${spriteUrl(iconName, theme)}")`;
  element.style.backgroundSize = "contain";
  element.style.backgroundPosition = "center";
  element.style.backgroundRepeat = "no-repeat";
}

export function applyWeatherIcon(element, iconName, theme) {
  if (!element) return;
  element.style.width = "52px";
  element.style.height = "52px";
  element.style.backgroundImage = `url("${weatherIconUrl(iconName, theme)}")`;
  element.style.backgroundSize = "contain";
  element.style.backgroundPosition = "center";
  element.style.backgroundRepeat = "no-repeat";
}
