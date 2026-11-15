// End-to-end check of the real component tree.
//
// This mounts the whole <App/> against the real fixtures through the real
// store, parser and chart. It is the check the old smoke.mjs was for: it
// catches a blank page, a broken session load, or a chart that produced no
// marks. It cannot catch a visual bug.
//
// (The original plan ran this in jsdom against the built bundle, but jsdom
// does not execute `<script type="module">`, so the built page never ran
// there. Mounting the real tree in vitest tests the same behaviour and
// actually executes.)

import { describe, it, expect, beforeAll, vi } from "vitest";
import { render, screen, waitFor, fireEvent } from "@testing-library/react";
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
    hook: "Secret in source, .env touched, force-push, five rewrites of one file.",
    severity: "critical",
    file: "risky-session.json",
  },
]);

beforeAll(() => {
  vi.stubGlobal(
    "fetch",
    vi.fn(async (input: RequestInfo | URL) => {
      const url = String(input);
      if (url.includes("index.json")) {
        return new Response(manifest, { status: 200 });
      }
      if (url.includes("risky-session.json")) {
        return new Response(risky, { status: 200 });
      }
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

describe("the assembled page", () => {
  it("loads the real session and renders the instrument", async () => {
    render(<App />);

    // The hero is there before the session arrives.
    expect(screen.getByRole("heading", { level: 1 })).toBeTruthy();

    // The session loads from the real fixture.
    await waitFor(
      () => expect(screen.getAllByText(/webhook signature check/i).length).toBeGreaterThan(0),
      { timeout: 4000 },
    );

    // The chart's tracks and marks come from the real events. The marks only
    // appear once the view has been sized to the session, so wait for them
    // rather than for the tracks (which render on the first frame).
    await waitFor(
      () => {
        expect(document.querySelectorAll("g.track").length).toBeGreaterThanOrEqual(5);
        expect(document.querySelectorAll(".mark-prompt").length).toBeGreaterThan(0);
        expect(document.querySelectorAll(".mark-incident").length).toBeGreaterThan(0);
      },
      { timeout: 4000 },
    );
    expect(document.querySelector('[role="slider"]')).not.toBeNull();

    // The report is populated from the real incidents.
    expect(screen.getByText(/incident report/i)).toBeTruthy();
    expect(screen.getAllByText(/verdict/i).length).toBeGreaterThan(0);

    // A real file path from the fixture appears.
    expect(document.body.textContent).toMatch(/src\//);
  });

  it("opens the drawer with a diff when an incident is clicked", async () => {
    render(<App />);

    await waitFor(
      () => expect(document.querySelector(".mark-incident")).not.toBeNull(),
      { timeout: 4000 },
    );

    const marker = document.querySelector(".mark-incident")!;
    fireEvent.click(marker);

    await waitFor(() => {
      expect(document.body.textContent).toMatch(/cockpit voice recorder|diff|caused by/i);
    });
  });
});
