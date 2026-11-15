import { describe, it, expect } from "vitest";
import {
  TRACK_HEIGHT,
  domain,
  xFor,
  tFor,
  churnSeries,
  type View,
} from "./layout";
import type { Session } from "../types";
import weakened from "../../../fixtures/weakened-tests.json";
import clean from "../../../fixtures/clean.json";

const session = weakened as unknown as Session;

describe("TRACK_HEIGHT", () => {
  it("matches the six tracks in the spec", () => {
    expect(TRACK_HEIGHT).toEqual({
      axis: 32,
      prompts: 48,
      files: 100,
      tests: 48,
      commands: 32,
      incidents: 48,
    });
  });
});

describe("domain", () => {
  it("spans 0..duration, not the absolute epoch (Review Focus 1)", () => {
    const d = domain(session);
    expect(d.start).toBe(0);
    expect(d.end).toBe(99000);
  });

  it("falls back to the last relative step when duration_ms is 0", () => {
    const d = domain({
      meta: { duration_ms: 0 },
      steps: [
        { seq: 0, t_ms: 1794653100000, kind: "session_start" },
        { seq: 1, t_ms: 4200, kind: "note" },
      ],
    } as unknown as Session);
    expect(d.end).toBe(4200);
  });

  it("survives a single-step session (Review Focus 5)", () => {
    const d = domain({
      meta: { duration_ms: 0 },
      steps: [{ seq: 0, t_ms: 0, kind: "session_start" }],
    } as unknown as Session);
    expect(Number.isFinite(d.end)).toBe(true);
    expect(d.end).toBeGreaterThanOrEqual(0);
  });
});

describe("churnSeries", () => {
  it("never plots the session_start step", () => {
    const points = churnSeries(session);
    expect(Math.max(...points.map((p) => p.t))).toBeLessThan(1e6);
  });

  it("starts at zero churn", () => {
    expect(churnSeries(session)[0]).toEqual({ t: 0, churn: 0 });
  });

  it("accumulates lines added and removed", () => {
    // Verified against the fixture: 2 + 1 + 2 + 1 = 6, not 2 per change.
    const points = churnSeries(session);
    const last = points[points.length - 1];
    expect(last.churn).toBe(6);
  });

  it("returns a single point for a session with no file changes", () => {
    const points = churnSeries({
      meta: { duration_ms: 1000 },
      steps: [{ seq: 0, t_ms: 0, kind: "session_start" }],
    } as unknown as Session);
    expect(points).toEqual([{ t: 0, churn: 0 }]);
  });

  it("is finite for clean.json", () => {
    const points = churnSeries(clean as unknown as Session);
    expect(points.every((p) => Number.isFinite(p.churn))).toBe(true);
  });
});

describe("xFor / tFor", () => {
  const view: View = { t0: 0, t1: 1000, width: 500 };

  it("maps the ends of the domain", () => {
    expect(xFor(0, view)).toBe(0);
    expect(xFor(1000, view)).toBe(500);
    expect(xFor(500, view)).toBe(250);
  });

  it("round-trips", () => {
    expect(tFor(xFor(370, view), view)).toBeCloseTo(370, 6);
  });

  it("does not divide by zero on a degenerate view", () => {
    const flat: View = { t0: 5, t1: 5, width: 500 };
    expect(Number.isFinite(xFor(5, flat))).toBe(true);
  });

  it("maps a step's time through the incident's step index (Review Focus 2)", () => {
    const incident = session.incidents[0]; // step: 4
    const step = session.steps[incident.step];
    expect(step.kind).toBe("test_run");
    expect(step.t_ms).toBe(16000);
  });
});
