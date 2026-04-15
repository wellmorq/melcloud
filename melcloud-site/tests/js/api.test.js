import assert from "node:assert/strict";
import { test } from "node:test";
import { defaultTimeoutMs, fetchJson } from "../../public/js/api.js";

test("fetchJson returns parsed JSON for successful responses", async () => {
  const payload = await fetchJson("/ok", {}, { fetchImpl: jsonResponse(200, { ok: true }) });
  assert.deepEqual(payload, { ok: true });
});

test("fetchJson reports backend JSON errors", async () => {
  await assert.rejects(
    fetchJson("/bad", {}, {
      fetchImpl: jsonResponse(502, {
        error: "MELCloud command failed.",
        kind: "cli",
        details: "raw stderr",
      }),
    }),
    /MELCloud command failed/,
  );
});

test("fetchJson reports non-JSON response status text", async () => {
  await assert.rejects(
    fetchJson("/bad", {}, { fetchImpl: textResponse(500, "broken", "Internal Server Error") }),
    /Internal Server Error/,
  );
});

test("fetchJson aborts hanging requests", async () => {
  const fetchImpl = (_url, options) =>
    new Promise((_resolve, reject) => {
      options.signal.addEventListener("abort", () => {
        reject(new DOMException("aborted", "AbortError"));
      });
    });

  await assert.rejects(fetchJson("/slow", {}, { fetchImpl, timeoutMs: 1 }), /timed out/);
});

test("frontend timeout stays above default backend CLI timeout", () => {
  assert.ok(defaultTimeoutMs > 90_000);
});

function jsonResponse(status, payload) {
  return async () =>
    new Response(JSON.stringify(payload), {
      status,
      statusText: status === 200 ? "OK" : "Bad Gateway",
      headers: { "Content-Type": "application/json" },
    });
}

function textResponse(status, text, statusText) {
  return async () => new Response(text, { status, statusText });
}
