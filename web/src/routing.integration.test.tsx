// The route must load the session it names.
//
// This is the test the reviewer asked for: it loads a NON-default session by
// hash and asserts the rendered title. Without the fix, the store loaded
// "risky-session" regardless of the hash, so the URL, the picker's
// aria-pressed state and the screen all disagreed.

import { describe, it, expect, beforeAll, vi } from "vitest";
import { render, waitFor } from "@testing-library/react";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { App } from "./App";

const clean = readFileSync(resolve(__dirname, "../../fixtures/clean.json"), "utf8");
const risky = readFileSync(
  resolve(__dirname, "../../fixtures/risky-session.json"),
  "utf8",
);

const manifest = JSON.stringify([
  {
    id: "clean",
    title: "Checkout: split the store",
    hook: "A quiet session.",
    severity: "info",
    file: "clean.json",
  },
  {
    id: "risky-session",
    title: "Auth: ship the webhook signature check",
    hook: "Secret in source.",
    severity: "critical",
    file: "risky-session.json",
  },
]);

beforeAll(() => {
  window.location.hash = "#/session/clean";

  vi.stubGlobal(
    "fetch",
    vi.fn(async (input: RequestInfo | URL) => {
      const url = String(input);
      if (url.includes("index.json")) return new Response(manifest, { status: 200 });
      if (url.includes("clean.json")) return new Response(clean, { status: 200 });
      if (url.includes("risky-session.json")) return new Response(risky, { status: 200 });
      return new Response("not found", { status: 404 });
    }),
  );

  vi.stubGlobal(
    "ResizeObserver",
    class {
      observe() {}
      unobserve() {}
      disconnect() {}
    },
  );

  vi.stubGlobal(
    "IntersectionObserver",
    class {
      constructor(private callback: IntersectionObserverCallback) {}
      observe(target: Element) {
        this.callback(
          [{ isIntersecting: true, target } as IntersectionObserverEntry],
          this as unknown as IntersectionObserver,
        );
      }
      unobserve() {}
      disconnect() {}
    },
  );

  vi.stubGlobal("matchMedia", (query: string) => ({
    matches: false,
    media: query,
    addEventListener() {},
    removeEventListener() {},
    addListener() {},
    removeListener() {},
  }));
});

describe("deep links", () => {
  it("loads the session named in the hash, not the default", async () => {
    render(<App />);

    // The report is populated from the LOADED session, so it is the honest
    // place to assert. The catalogue (SessionPicker) lists every session by
    // design, so a whole-body check would match the default's title there
    // whether or not the deep link worked.
    const reportText = () =>
      document.getElementById("report")?.textContent ?? "";

    await waitFor(
      () => {
        expect(reportText()).toMatch(/split the store/i);
      },
      { timeout: 4000 },
    );

    // And the report must NOT be the default session's.
    expect(reportText()).not.toMatch(/webhook signature check/i);
    // The clean session's verdict, which the risky one never shows.
    expect(reportText()).toMatch(/no incidents detected/i);
  });
});
