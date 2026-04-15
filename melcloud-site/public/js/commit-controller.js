import { mergePatch } from "./domain.js";

export class CommitController {
  constructor(options) {
    this.getDebounceMs = options.getDebounceMs;
    this.getActivePresetId = options.getActivePresetId;
    this.patchPresetRequest = options.patchPresetRequest;
    this.onSnapshot = options.onSnapshot;
    this.onBusyStart = options.onBusyStart;
    this.onBusyEnd = options.onBusyEnd;
    this.onError = options.onError;
    this.onClearError = options.onClearError;
    this.onRecover = options.onRecover;
    this.onRender = options.onRender;
    this.onStatusChange = options.onStatusChange || (() => {});
    this.setTimeoutFn = options.setTimeoutFn || window.setTimeout.bind(window);
    this.clearTimeoutFn = options.clearTimeoutFn || window.clearTimeout.bind(window);

    this.status = "idle";
    this.pendingPresetId = null;
    this.pendingPatch = null;
    this.flushTimer = null;
    this.flushTaskQueued = false;
    this.networkQueue = Promise.resolve();
  }

  queuePreset(presetId, presetConfig) {
    this.clearPending({ setIdle: false });
    this.pendingPresetId = presetId;
    this.pendingPatch = mergePatch(null, presetConfig || {});
    this.schedule();
  }

  queuePatch(patch) {
    this.pendingPatch = mergePatch(this.pendingPatch, patch);
    this.schedule();
  }

  clearPending(options = {}) {
    if (this.flushTimer != null) {
      this.clearTimeoutFn(this.flushTimer);
    }
    this.flushTimer = null;
    this.pendingPresetId = null;
    this.pendingPatch = null;
    if (options.setIdle !== false && !this.flushTaskQueued) {
      this.setStatus("idle");
    }
  }

  schedule() {
    if (this.flushTimer != null) {
      this.clearTimeoutFn(this.flushTimer);
    }
    this.setStatus("debouncing");
    this.flushTimer = this.setTimeoutFn(() => this.flush(), Math.max(0, this.getDebounceMs()));
  }

  async flush() {
    this.flushTimer = null;
    if (!this.pendingPatch || this.flushTaskQueued) {
      return;
    }
    this.flushTaskQueued = true;
    await this.enqueueNetworkTask(async () => {
      this.flushTaskQueued = false;
      const presetId = this.pendingPresetId ?? this.getActivePresetId();
      const patch = this.pendingPatch;
      if (!presetId) {
        this.setStatus("idle");
        return;
      }

      this.pendingPresetId = null;
      this.pendingPatch = null;
      this.setStatus("syncing");
      this.onBusyStart();
      let writeSucceeded = false;
      try {
        this.onSnapshot(await this.patchPresetRequest(presetId, patch));
        writeSucceeded = true;
        this.onClearError();
      } catch (error) {
        this.onError(error);
        const recovered = await this.onRecover(error);
        if (!this.pendingPatch && !this.pendingPresetId) {
          this.setStatus(recovered ? "idle" : "unsynced");
        }
      } finally {
        this.onBusyEnd();
        this.onRender();
        if (this.pendingPatch || this.pendingPresetId) {
          this.schedule();
        } else if (writeSucceeded) {
          this.setStatus("idle");
        }
      }
    });
  }

  async enqueueNetworkTask(task) {
    const queued = this.networkQueue.catch(() => {}).then(task);
    this.networkQueue = queued;
    return queued;
  }

  setStatus(nextStatus) {
    if (this.status === nextStatus) return;
    this.status = nextStatus;
    this.onStatusChange(nextStatus);
  }
}
