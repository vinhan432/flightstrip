import { describe, it, expect } from "vitest";
import { verdictOf, probableCause, recommendations } from "./IncidentReport";
import type { Session } from "../types";
import weakened from "../../../fixtures/weakened-tests.json";
import clean from "../../../fixtures/clean.json";
import risky from "../../../fixtures/risky-session.json";

const weakenedS = weakened as unknown as Session;
const cleanS = clean as unknown as Session;
const riskyS = risky as unknown as Session;

describe("verdictOf", () => {
  it("calls a critical session critical", () => {
    expect(verdictOf(weakenedS).level).toBe("critical");
  });

  it("calls a clean session clean", () => {
    expect(verdictOf(cleanS).level).toBe("info");
  });

  it("handles zero incidents without throwing (Review Focus 5)", () => {
    expect(verdictOf({ incidents: [], stats: {} } as unknown as Session).level).toBe(
      "info",
    );
  });

  it("produces a headline naming the count", () => {
    expect(verdictOf(weakenedS).headline).toMatch(/3/);
  });
});

describe("probableCause", () => {
  it("names the dominant incident kind", () => {
    expect(probableCause(weakenedS)).toMatch(/test/i);
  });

  it("returns a sentence for a clean session", () => {
    expect(probableCause(cleanS).length).toBeGreaterThan(0);
  });
});

describe("recommendations", () => {
  it("gives one line per kind present", () => {
    const recs = recommendations(weakenedS);
    expect(recs.length).toBeGreaterThan(0);
  });

  it("gives nothing to do for a clean session", () => {
    expect(recommendations(cleanS).length).toBe(0);
  });

  it("covers the risky session's kinds", () => {
    const recs = recommendations(riskyS);
    expect(recs.length).toBeGreaterThanOrEqual(3);
  });
});
