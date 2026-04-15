export const layoutBreakpointPx = 1180;
export const themeStorageKey = "melcloud-site-theme";

export function selectLayout(width) {
  return width >= layoutBreakpointPx ? "horizontal" : "vertical";
}

export function loadTheme(storage = window.localStorage, media = window.matchMedia) {
  try {
    const saved = storage.getItem(themeStorageKey);
    if (saved === "dark" || saved === "light") return saved;
  } catch {
    return "light";
  }
  return media("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

export function saveTheme(theme, storage = window.localStorage) {
  try {
    storage.setItem(themeStorageKey, theme);
  } catch {
    // Theme persistence is best-effort; rendering should not depend on storage.
  }
}

export function applyTheme(root, body, theme) {
  root.dataset.theme = theme;
  body.dataset.theme = theme;
}

export function applyLayout(root, appShell, width) {
  const layout = selectLayout(width);
  root.dataset.layout = layout;
  appShell.dataset.layout = layout;
  return layout;
}

export function setReadyState(root, isReady) {
  root.dataset.ready = String(isReady);
}

export function setBusyState(root, body, isBusy) {
  const busy = String(isBusy);
  root.dataset.busy = busy;
  body.dataset.busy = busy;
}
