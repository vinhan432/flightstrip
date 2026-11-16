import { describe, it, expect } from "vitest";
import { zoomView, resetView, type ViewRect } from "./view";

const domain = { start: 0, end: 99000 };
const base: ViewRect = { t0: 0, t1: 99000, width: 1200 };

describe("zoomView", () => {
  it("narrows the span when zooming in", () => {
    const next = zoomView(base, 0.8, domain);
    expect(next.t1 - next.t0).toBeLessThan(base.t1 - base.t0);
  });

  it("widens the span when zooming out from a zoomed-in view", () => {
    const zoomedIn = { ...base, t0: 40000, t1: 60000 };
    const next = zoomView(zoomedIn, 1.25, domain);
    expect(next.t1 - next.t0).toBeGreaterThan(zoomedIn.t1 - zoomedIn.t0);
  });

  it("does not widen past the full domain", () => {
    const next = zoomView(base, 1.25, domain);
    expect(next.t1 - next.t0).toBe(domain.end - domain.start);
  });

  it("never goes below the session start", () => {
    const zoomed = zoomView({ ...base, t0: 0, t1: 5000 }, 1.25, domain);
    expect(zoomed.t0).toBeGreaterThanOrEqual(domain.start);
  });

  it("never goes past the session end", () => {
    const zoomed = zoomView({ ...base, t0: 94000, t1: 99000 }, 1.25, domain);
    expect(zoomed.t1).toBeLessThanOrEqual(domain.end);
  });

  it("keeps the span inside the domain when zooming out at an edge", () => {
    const zoomed = zoomView({ ...base, t0: 0, t1: 10000 }, 4, domain);
    expect(zoomed.t0).toBeGreaterThanOrEqual(domain.start);
    expect(zoomed.t1).toBeLessThanOrEqual(domain.end);
  });

  it("preserves the width", () => {
    expect(zoomView(base, 0.8, domain).width).toBe(1200);
  });
});

describe("resetView", () => {
  it("restores the full domain", () => {
    const reset = resetView(domain, 1200);
    expect(reset.t0).toBe(0);
    expect(reset.t1).toBe(99000);
  });

  it("restores from a zoomed-in view", () => {
    const reset = resetView(domain, 1200);
    expect(reset.t1 - reset.t0).toBe(99000);
  });

  it("never produces a zero-width span", () => {
    const reset = resetView({ start: 0, end: 0 }, 1200);
    expect(reset.t1).toBeGreaterThan(reset.t0);
  });
});
