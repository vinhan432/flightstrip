import { describe, it, expect } from "vitest";
import { advance, nextStepTime, prevStepTime } from "./usePlayback";
import type { Session } from "../types";
import weakened from "../../../fixtures/weakened-tests.json";

const session = weakened as unknown as Session;

describe("advance", () => {
  it("advances by dt times speed", () =>
    expect(advance(1000, 500, 2, 99000)).toBe(2000));
  it("clamps at the duration", () =>
    expect(advance(99000, 500, 4, 99000)).toBe(99000));
  it("never goes below zero", () => expect(advance(0, -500, 1, 99000)).toBe(0));
  it("handles a zero duration", () => expect(advance(0, 500, 1, 0)).toBe(0));
});

describe("nextStepTime / prevStepTime", () => {
  it("finds the next relative step time", () => {
    // Step 0 is the absolute anchor; the next real time after 0 is 1500.
    expect(nextStepTime(session, 0)).toBe(1500);
  });

  it("finds the previous step time", () => {
    expect(prevStepTime(session, 16000)).toBe(10000);
  });

  it("stays put at the end", () => {
    expect(nextStepTime(session, 99000)).toBe(99000);
  });

  it("stays put at the start", () => {
    expect(prevStepTime(session, 0)).toBe(0);
  });

  it("never returns the absolute epoch (Review Focus 1)", () => {
    const times = session.steps
      .map((s) => s.t_ms)
      .filter((t) => t < 1e6);
    expect(Math.max(...times)).toBeLessThan(1e6);
  });
});
