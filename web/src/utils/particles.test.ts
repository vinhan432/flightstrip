import { describe, it, expect } from "vitest";
import { makeParticles, step, repel, connections } from "./particles";

describe("makeParticles", () => {
  it("makes the requested count inside the box", () => {
    const ps = makeParticles(100, 800, 600);
    expect(ps).toHaveLength(100);
    expect(ps.every((p) => p.x >= 0 && p.x <= 800)).toBe(true);
    expect(ps.every((p) => p.y >= 0 && p.y <= 600)).toBe(true);
  });

  it("uses the spec's slow drift", () => {
    const ps = makeParticles(50, 800, 600);
    for (const p of ps) {
      expect(Math.abs(p.vx)).toBeGreaterThanOrEqual(0.1);
      expect(Math.abs(p.vx)).toBeLessThanOrEqual(0.3);
      expect(p.vy).toBeLessThanOrEqual(-0.1);
      expect(p.vy).toBeGreaterThanOrEqual(-0.3);
      expect(p.radius).toBeGreaterThanOrEqual(1);
      expect(p.radius).toBeLessThanOrEqual(2);
    }
  });
});

describe("step", () => {
  it("wraps around the right edge", () => {
    const p = { x: 799, y: 10, vx: 5, vy: 0, radius: 1, opacity: 1 };
    step(p, 800, 600, 16);
    expect(p.x).toBeGreaterThanOrEqual(0);
    expect(p.x).toBeLessThan(800);
  });

  it("wraps around the top edge", () => {
    const p = { x: 10, y: 1, vx: 0, vy: -5, radius: 1, opacity: 1 };
    step(p, 800, 600, 16);
    expect(p.y).toBeGreaterThanOrEqual(0);
    expect(p.y).toBeLessThan(600);
  });

  it("is frame-rate independent", () => {
    const a = { x: 100, y: 100, vx: 0.2, vy: -0.2, radius: 1, opacity: 1 };
    const b = { x: 100, y: 100, vx: 0.2, vy: -0.2, radius: 1, opacity: 1 };
    step(a, 800, 600, 16);
    step(b, 800, 600, 32);
    expect(b.x - 100).toBeCloseTo(2 * (a.x - 100), 5);
  });
});

describe("repel", () => {
  it("pushes a particle away from the cursor", () => {
    const p = { x: 100, y: 100, vx: 0, vy: 0, radius: 1, opacity: 1 };
    repel(p, 110, 100, 150);
    expect(p.vx).toBeLessThan(0);
  });

  it("leaves a distant particle alone", () => {
    const p = { x: 100, y: 100, vx: 0, vy: 0, radius: 1, opacity: 1 };
    repel(p, 900, 900, 150);
    expect(p.vx).toBe(0);
    expect(p.vy).toBe(0);
  });

  it("does not divide by zero when the cursor is on the particle", () => {
    const p = { x: 100, y: 100, vx: 0, vy: 0, radius: 1, opacity: 1 };
    repel(p, 100, 100, 150);
    expect(Number.isFinite(p.vx)).toBe(true);
    expect(Number.isFinite(p.vy)).toBe(true);
  });
});

describe("connections", () => {
  it("connects only pairs within the distance", () => {
    const ps = [
      { x: 0, y: 0, vx: 0, vy: 0, radius: 1, opacity: 1 },
      { x: 50, y: 0, vx: 0, vy: 0, radius: 1, opacity: 1 },
      { x: 500, y: 0, vx: 0, vy: 0, radius: 1, opacity: 1 },
    ];
    expect(connections(ps, 100)).toEqual([[0, 1]]);
  });

  it("returns nothing for a single particle", () => {
    expect(
      connections([{ x: 0, y: 0, vx: 0, vy: 0, radius: 1, opacity: 1 }], 100),
    ).toEqual([]);
  });
});
