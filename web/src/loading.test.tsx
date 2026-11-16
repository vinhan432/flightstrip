// The session must load exactly once.
//
// The bug this exists to prevent: `useSession` decided "is this session
// already loaded?" by comparing the manifest's id (`risky-session`) with the
// session's own `meta.id` (`20261114T133000Z-c3`). Those never match, so the
// effect refetched on every render — an infinite load loop that left the
// chart stuck on its skeleton forever.
//
// The earlier integration tests passed anyway, because the text they waited
// for also appears in the session picker. This one counts the fetches.

import { describe, it, expect, beforeAll, vi, afterEach } from "vitest";
import { render, waitFor, cleanup } from "@testing-library/react";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { App } from "./App";

const risky = readFileSync(
  resolve(__dirname, "../../fixtures/risky-session.json"),
  "utf8",
);

const manifest = JSON.stringify([
  {
    id: "risky-session",
    title: "Auth: ship the webhook signature check",
    hook: "Secret in source.",
    severity: "critical",
    file: "risky-session.json",
  },
]);

let fetchSpy: ReturnType<typeof vi.fn>;

beforeAll(() => {
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

afterEach(() => cleanup());

describe("session loading", () => {
  it("fetches the session once, not on every render", async () => {
    window.location.hash = "#/s/risky-session";

    fetchSpy = vi.fn(async (input: RequestInfo | URL) => {
      const url = String(input);
      if (url.includes("index.json")) return new Response(manifest, { status: 200 });
      if (url.includes("risky-session.json")) return new Response(risky, { status: 200 });
      return new Response("not found", { status: 404 });
    });
    vi.stubGlobal("fetch", fetchSpy);

    const { container } = render(<App />);

    // The chart leaves the skeleton and renders its tracks.
    await waitFor(
      () => {
        expect(container.querySelector(".skeleton")).toBeNull();
        expect(container.querySelectorAll("g.track").length).toBeGreaterThan(0);
      },
      { timeout: 4000 },
    );

    // Give any runaway loop time to show itself.
    await new Promise((r) => setTimeout(r, 300));

    const sessionFetches = fetchSpy.mock.calls.filter(([input]) =>
      String(input).includes("risky-session.json"),
    ).length;

    expect(sessionFetches).toBe(1);
  });
});
