import { describe, it, expect } from "vitest";
import { tracePoints } from "./CanvasTrace";
import type { Session } from "../../types";
import weakened from "../../../../fixtures/weakened-tests.json";

const session = weakened as unknown as Session;
const view = { t0: 0, t1: 99000, width: 1200 };

describe("tracePoints", () => {
  it("returns points inside the box", () => {
    const pts = tracePoints(session, view, 600);
    expect(pts.length).toBeGreaterThan(1);
    expect(pts.every((p) => p.x >= 0 && p.x <= 1200)).toBe(true);
    expect(pts.every((p) => p.y >= 0 && p.y <= 100)).toBe(true);
  });

  it("is flat and finite for a session with no file changes (Review Focus 5)", () => {
    const pts = tracePoints(
      { meta: { duration_ms: 1000 }, steps: [] } as unknown as Session,
      { t0: 0, t1: 1000, width: 100 },
      100,
    );
    expect(pts.length).toBeGreaterThan(0);
    expect(pts.every((p) => Number.isFinite(p.y))).toBe(true);
  });

  it("rises left to right, since churn only accumulates", () => {
    const pts = tracePoints(session, view, 600);
    const first = pts[0];
    const last = pts[pts.length - 1];
    expect(last.y).toBeLessThanOrEqual(first.y);
  });

  it("is finite for a degenerate view", () => {
    const pts = tracePoints(session, { t0: 0, t1: 0, width: 0 }, 10);
    expect(pts.every((p) => Number.isFinite(p.x) && Number.isFinite(p.y))).toBe(
      true,
    );
  });
});
