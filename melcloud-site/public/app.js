import { fetchJson } from "./js/api.js";
import { CommitController } from "./js/commit-controller.js";
import {
  applyOptimisticPatch,
  applyOptimisticPreset,
  clampTemperature,
  currentTargetTemperature,
  currentTemperatureBounds,
  temperatureStep,
} from "./js/domain.js";
import { translate } from "./js/i18n.js";
import {
  applyLayout,
  applyTheme,
  loadTheme,
  saveTheme,
  setBusyState,
  setReadyState,
} from "./js/layout.js";
import {
  clearBanner,
  createRenderer,
  readElements,
  showBanner,
  sliderTemperature,
  updateSyncStatus,
} from "./js/render.js";

const state = {
  snapshot: null,
  lastConfirmedSnapshot: null,
  busyCount: 0,
  theme: loadTheme(),
  syncStatus: "idle",
  tempDraft: null,
  tempDraftCommitTimer: null,
  commitController: null,
};

const elements = readElements();
const renderer = createRenderer(elements, {
  applyPreset,
  mutateActive,
});

init();

async function init() {
  applyTheme(document.documentElement, document.body, state.theme);
  syncLayout();
  bindEvents();
  state.commitController = createCommitController();
  setSyncStatus("idle");
  try {
    await refreshSnapshot();
  } finally {
    setReadyState(document.documentElement, true);
  }
  window.addEventListener("resize", syncLayout);
}

function bindEvents() {
  elements.themeToggle.addEventListener("click", toggleTheme);
  elements.powerToggle.addEventListener("click", () => {
    if (!state.snapshot) return;
    mutateActive({ power: !state.snapshot.live_status.power });
  });
  elements.tempMinus.addEventListener("click", () => adjustTemperature(-1));
  elements.tempPlus.addEventListener("click", () => adjustTemperature(1));
  elements.fanAuto.addEventListener("click", () => mutateActive({ fan_speed: "auto" }));
  elements.vaneHorizontalAuto.addEventListener("click", () => mutateActive({ vane_horizontal: "auto" }));
  elements.vaneVerticalAuto.addEventListener("click", () => mutateActive({ vane_vertical: "auto" }));
  bindSlider();
}

function createCommitController() {
  return new CommitController({
    getDebounceMs: currentDebounceMs,
    getActivePresetId: () => state.snapshot?.active_preset_id,
    patchPresetRequest: (presetId, patch) =>
      fetchJson(`/api/presets/${presetId}/config`, {
        method: "PATCH",
        body: JSON.stringify(patch),
      }),
    onSnapshot: syncSnapshot,
    onBusyStart: beginBusy,
    onBusyEnd: endBusy,
    onError: (error) => showError(error),
    onClearError: () => clearBanner(elements),
    onRecover: recoverSnapshot,
    onRender: render,
    onStatusChange: setSyncStatus,
  });
}

function toggleTheme() {
  state.theme = state.theme === "dark" ? "light" : "dark";
  saveTheme(state.theme);
  applyTheme(document.documentElement, document.body, state.theme);
  render();
}

function syncLayout() {
  applyLayout(document.documentElement, elements.appShell, window.innerWidth);
}

function bindSlider() {
  let dragging = false;

  const update = (event) => {
    if (!state.snapshot) return;
    state.tempDraft = sliderTemperature(
      elements,
      state.snapshot,
      event.clientY,
      clampTemperature,
      currentTemperatureBounds(state.snapshot),
    );
    render();
  };

  elements.tempSlider.addEventListener("pointerdown", (event) => {
    event.preventDefault();
    dragging = true;
    elements.tempSlider.setPointerCapture(event.pointerId);
    update(event);
  });
  elements.tempSlider.addEventListener("pointermove", (event) => {
    if (dragging) update(event);
  });
  elements.tempSlider.addEventListener("pointerup", (event) => {
    if (!dragging) return;
    dragging = false;
    elements.tempSlider.releasePointerCapture(event.pointerId);
    update(event);
    commitDraftTemperature();
  });
  elements.tempSlider.addEventListener("pointercancel", () => {
    dragging = false;
    state.tempDraft = null;
    render();
  });
  elements.tempSlider.addEventListener(
    "wheel",
    (event) => {
      if (!state.snapshot) return;
      event.preventDefault();
      const direction = event.deltaY < 0 ? 1 : -1;
      state.tempDraft = clampTemperature(
        currentTargetTemperature(state.snapshot, state.tempDraft) + direction * temperatureStep(state.snapshot),
        state.snapshot,
      );
      render();
      window.clearTimeout(state.tempDraftCommitTimer);
      state.tempDraftCommitTimer = window.setTimeout(commitDraftTemperature, 180);
    },
    { passive: false },
  );
}

async function refreshSnapshot() {
  await runImmediateRequest(() => fetchJson("/api/state"));
}

async function applyPreset(presetId) {
  clearPendingConfig();
  const presetConfig = state.snapshot?.presets?.find((preset) => preset.id === presetId)?.config || {};
  state.commitController.queuePreset(presetId, presetConfig);
  state.snapshot = applyOptimisticPreset(state.snapshot, presetId);
  clearBanner(elements);
  render();
}

async function mutateActive(patch) {
  const activePreset = state.snapshot?.active_preset_id;
  if (!activePreset) {
    showBanner(elements, t("choosePreset"), true);
    return;
  }
  state.snapshot = applyOptimisticPatch(state.snapshot, patch);
  state.commitController.queuePatch(patch);
  clearBanner(elements);
  render();
}

async function runImmediateRequest(request) {
  try {
    beginBusy();
    syncSnapshot(await request(), { confirmed: true });
    setSyncStatus("idle");
    render();
    clearBanner(elements);
  } catch (error) {
    setSyncStatus("unsynced");
    showError(error);
  } finally {
    endBusy();
  }
}

function clearPendingConfig() {
  window.clearTimeout(state.tempDraftCommitTimer);
  state.tempDraftCommitTimer = null;
  state.tempDraft = null;
  state.commitController.clearPending();
}

async function recoverSnapshot() {
  try {
    syncSnapshot(await fetchJson("/api/state"), { confirmed: true });
    return true;
  } catch {
    rollbackToLastConfirmed();
    showBanner(elements, t("syncRecoveryFailed"), true);
    return false;
  }
}

function syncSnapshot(serverSnapshot, options = {}) {
  if (options.confirmed !== false) {
    state.lastConfirmedSnapshot = structuredClone(serverSnapshot);
  }
  state.snapshot = structuredClone(serverSnapshot);
  state.tempDraft = null;
  if (state.commitController?.pendingPresetId) {
    state.snapshot = applyOptimisticPreset(state.snapshot, state.commitController.pendingPresetId);
  }
  if (state.commitController?.pendingPatch) {
    state.snapshot = applyOptimisticPatch(state.snapshot, state.commitController.pendingPatch);
  }
}

function rollbackToLastConfirmed() {
  if (!state.lastConfirmedSnapshot) return;
  state.snapshot = structuredClone(state.lastConfirmedSnapshot);
  state.tempDraft = null;
}

function currentDebounceMs() {
  return Math.max(0, state.snapshot?.commit_debounce_ms ?? 3000);
}

function render() {
  if (!state.snapshot) return;
  renderer.render(state.snapshot, {
    theme: state.theme,
    targetTemperature: currentTargetTemperature(state.snapshot, state.tempDraft),
    temperatureBounds: currentTemperatureBounds(state.snapshot),
    translate: t,
  });
  setSyncStatus(state.syncStatus);
  syncBusyState();
}

function adjustTemperature(direction) {
  if (!state.snapshot) return;
  mutateActive({
    target_temperature: clampTemperature(
      currentTargetTemperature(state.snapshot, state.tempDraft) + direction * temperatureStep(state.snapshot),
      state.snapshot,
    ),
  });
}

function commitDraftTemperature() {
  state.tempDraftCommitTimer = null;
  if (state.tempDraft == null || !state.snapshot) return;
  const next = state.tempDraft;
  state.tempDraft = null;
  if (Math.abs((state.snapshot.live_status.target_temperature ?? next) - next) < 0.01) {
    render();
    return;
  }
  mutateActive({ target_temperature: next });
}

function t(key, language = state.snapshot?.language || "en") {
  return translate(key, language);
}

function showError(error) {
  console.error(error);
  showBanner(elements, error?.message || String(error), true);
}

function setSyncStatus(status) {
  state.syncStatus = status;
  const key =
    status === "unsynced"
      ? "syncUnsynced"
      : status === "debouncing" || status === "syncing"
        ? "syncSaving"
        : "syncSynced";
  updateSyncStatus(elements, status, t(key));
}

function beginBusy() {
  state.busyCount += 1;
  syncBusyState();
}

function endBusy() {
  state.busyCount = Math.max(0, state.busyCount - 1);
  syncBusyState();
}

function syncBusyState() {
  setBusyState(document.documentElement, document.body, state.busyCount > 0);
}
