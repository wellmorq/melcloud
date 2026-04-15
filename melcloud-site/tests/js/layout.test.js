import assert from "node:assert/strict";
import { test } from "node:test";
import { applyLayout, loadTheme, selectLayout } from "../../public/js/layout.js";

test("selectLayout uses the same fixed breakpoint for startup and runtime", () => {
  assert.equal(selectLayout(1179), "vertical");
  assert.equal(selectLayout(1180), "horizontal");
});

test("applyLayout syncs root and app shell attributes", () => {
  const root = { dataset: {} };
  const shell = { dataset: {} };

  assert.equal(applyLayout(root, shell, 1600), "horizontal");
  assert.equal(root.dataset.layout, "horizontal");
  assert.equal(shell.dataset.layout, "horizontal");
});

test("loadTheme falls back safely when storage throws", () => {
  const storage = {
    getItem() {
      throw new Error("blocked");
    },
  };
  const media = () => ({ matches: true });

  assert.equal(loadTheme(storage, media), "light");
});
