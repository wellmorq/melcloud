import assert from "node:assert/strict";
import { test } from "node:test";
import { CommitController } from "../../public/js/commit-controller.js";

test("debounce flush merges quick patch updates", async () => {
  const harness = createHarness();
  const controller = harness.controller();

  controller.queuePatch({ power: true });
  controller.queuePatch({ fan_speed: "3" });

  assert.equal(harness.timers.length, 1);
  assert.equal(harness.timers[0].ms, 3000);
  await harness.runTimer();

  assert.deepEqual(harness.calls, [["patch", "site-heat", { power: true, fan_speed: "3" }]]);
});

test("preset selection clears stale pending patch", async () => {
  const harness = createHarness();
  const controller = harness.controller();

  controller.queuePatch({ fan_speed: "5" });
  controller.queuePreset("site-cool", coolPresetConfig());
  controller.queuePatch({ fan_speed: "4" });
  await harness.runTimer();

  assert.deepEqual(harness.calls, [
    [
      "patch",
      "site-cool",
      {
        ...coolPresetConfig(),
        fan_speed: "4",
      },
    ],
  ]);
});

test("preset and fan before debounce flush as one desired state patch", async () => {
  const harness = createHarness();
  const controller = harness.controller();

  controller.queuePreset("site-cool", coolPresetConfig());
  controller.queuePatch({ fan_speed: "4" });
  await harness.runTimer();

  assert.deepEqual(harness.calls, [
    [
      "patch",
      "site-cool",
      {
        ...coolPresetConfig(),
        fan_speed: "4",
      },
    ],
  ]);
});

test("preset and multiple controls before debounce flush as one merged patch", async () => {
  const harness = createHarness();
  const controller = harness.controller();

  controller.queuePreset("site-cool", coolPresetConfig());
  controller.queuePatch({ fan_speed: "4" });
  controller.queuePatch({ target_temperature: 22.5 });
  controller.queuePatch({ vane_horizontal: "2" });
  await harness.runTimer();

  assert.deepEqual(harness.calls, [
    [
      "patch",
      "site-cool",
      {
        ...coolPresetConfig(),
        fan_speed: "4",
        target_temperature: 22.5,
        vane_horizontal: "2",
      },
    ],
  ]);
});

test("in-flight write does not drop a new pending patch", async () => {
  const deferred = createDeferred();
  const started = createDeferred();
  const harness = createHarness({
    patchPresetRequest: async (presetId, patch) => {
      harness.calls.push(["patch", presetId, patch]);
      if (patch.fan_speed === "2") {
        started.resolve();
        await deferred.promise;
      }
      return { ok: true };
    },
  });
  const controller = harness.controller();

  controller.queuePatch({ fan_speed: "2" });
  const firstFlush = harness.runTimer();
  await started.promise;
  controller.queuePatch({ power: false });
  deferred.resolve();
  await firstFlush;
  await harness.runTimer();

  assert.deepEqual(harness.calls, [
    ["patch", "site-heat", { fan_speed: "2" }],
    ["patch", "site-heat", { power: false }],
  ]);
});

test("failed write reports error and runs recovery", async () => {
  const harness = createHarness({
    patchPresetRequest: async () => {
      throw new Error("device did not converge");
    },
  });
  const controller = harness.controller();

  controller.queuePatch({ power: false });
  await harness.runTimer();

  assert.equal(harness.errors[0].message, "device did not converge");
  assert.equal(harness.recoveries, 1);
  assert.deepEqual(harness.busy, ["start", "end"]);
  assert.equal(controller.status, "idle");
});

test("status transitions through debounce, syncing, and idle on success", async () => {
  const harness = createHarness();
  const controller = harness.controller();

  controller.queuePatch({ power: false });
  await harness.runTimer();

  assert.deepEqual(harness.statuses, ["debouncing", "syncing", "idle"]);
});

test("failed write with failed recovery becomes unsynced", async () => {
  const harness = createHarness({
    patchPresetRequest: async () => {
      throw new Error("backend timeout");
    },
    onRecover: async () => {
      harness.recoveries += 1;
      return false;
    },
  });
  const controller = harness.controller();

  controller.queuePatch({ power: false });
  await harness.runTimer();

  assert.equal(controller.status, "unsynced");
  assert.deepEqual(harness.statuses, ["debouncing", "syncing", "unsynced"]);
});

test("failed in-flight write keeps a queued user action", async () => {
  const deferred = createDeferred();
  const started = createDeferred();
  const harness = createHarness({
    patchPresetRequest: async (presetId, patch) => {
      harness.calls.push(["patch", presetId, patch]);
      if (patch.fan_speed === "2") {
        started.resolve();
        await deferred.promise;
        throw new Error("first write failed");
      }
      return { ok: true };
    },
    onRecover: async () => false,
  });
  const controller = harness.controller();

  controller.queuePatch({ fan_speed: "2" });
  const firstFlush = harness.runTimer();
  await started.promise;
  controller.queuePatch({ power: false });
  deferred.resolve();
  await firstFlush;
  await harness.runTimer();

  assert.deepEqual(harness.calls, [
    ["patch", "site-heat", { fan_speed: "2" }],
    ["patch", "site-heat", { power: false }],
  ]);
  assert.equal(controller.status, "idle");
});

test("preset and fan queued during in-flight patch run as final desired state", async () => {
  const deferred = createDeferred();
  const started = createDeferred();
  const harness = createHarness({
    patchPresetRequest: async (presetId, patch) => {
      harness.calls.push(["patch", presetId, patch]);
      started.resolve();
      await deferred.promise;
      return { ok: true };
    },
  });
  const controller = harness.controller();

  controller.queuePatch({ fan_speed: "1" });
  const firstFlush = harness.runTimer();
  await started.promise;
  controller.queuePreset("site-dry", dryPresetConfig());
  controller.queuePatch({ fan_speed: "5" });
  deferred.resolve();
  await firstFlush;
  await harness.runTimer();

  assert.deepEqual(harness.calls, [
    ["patch", "site-heat", { fan_speed: "1" }],
    ["patch", "site-dry", { ...dryPresetConfig(), fan_speed: "5" }],
  ]);
});

function createHarness(overrides = {}) {
  const harness = {
    activePresetId: "site-heat",
    calls: [],
    snapshots: [],
    errors: [],
    busy: [],
    recoveries: 0,
    statuses: [],
    timers: [],
    controller() {
      return new CommitController({
        getDebounceMs: () => 3000,
        getActivePresetId: () => harness.activePresetId,
        patchPresetRequest: async (presetId, patch) => {
          harness.calls.push(["patch", presetId, patch]);
          return { ok: true, presetId, patch };
        },
        onSnapshot: (snapshot) => harness.snapshots.push(snapshot),
        onBusyStart: () => harness.busy.push("start"),
        onBusyEnd: () => harness.busy.push("end"),
        onError: (error) => harness.errors.push(error),
        onClearError: () => {},
        onRecover: async () => {
          harness.recoveries += 1;
          return true;
        },
        onRender: () => {},
        onStatusChange: (status) => harness.statuses.push(status),
        setTimeoutFn: (fn, ms) => {
          harness.timers = [{ fn, ms }];
          return 1;
        },
        clearTimeoutFn: () => {
          harness.timers = [];
        },
        ...overrides,
      });
    },
    async runTimer() {
      const timer = harness.timers.shift();
      assert.ok(timer, "expected a queued timer");
      await timer.fn();
    },
  };
  return harness;
}

function coolPresetConfig() {
  return {
    power: true,
    mode: "cool",
    target_temperature: 20.5,
    fan_speed: "3",
    vane_horizontal: "5",
    vane_vertical: "1",
  };
}

function dryPresetConfig() {
  return {
    power: true,
    mode: "dry",
  };
}

function createDeferred() {
  let resolve;
  const promise = new Promise((done) => {
    resolve = done;
  });
  return { promise, resolve };
}
