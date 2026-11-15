import { describe, it, expect } from "vitest";
import { parseSession } from "./parseSession";
import weakened from "../../../fixtures/weakened-tests.json";
import clean from "../../../fixtures/clean.json";
import risky from "../../../fixtures/risky-session.json";

describe("parseSession", () => {
  it("accepts every real fixture", () => {
    expect(parseSession(weakened).ok).toBe(true);
    expect(parseSession(clean).ok).toBe(true);
    expect(parseSession(risky).ok).toBe(true);
  });

  it("names the first bad field", () => {
    const r = parseSession({ version: 1, meta: { id: "x" } });
    expect(r.ok).toBe(false);
    if (!r.ok) expect(r.error).toMatch(/meta\.title/);
  });

  it("rejects a non-object", () => {
    expect(parseSession(null).ok).toBe(false);
    expect(parseSession("nope").ok).toBe(false);
    expect(parseSession([]).ok).toBe(false);
  });

  it("reports a missing top-level field by name", () => {
    const r = parseSession({ version: 1, meta: {}, prompts: [], steps: [] });
    expect(r.ok).toBe(false);
  });

  it("returns the session on success", () => {
    const r = parseSession(weakened);
    expect(r.ok).toBe(true);
    if (r.ok) {
      expect(r.session.meta.title).toBe(
        "Uploads: make the failing test pass",
      );
      expect(r.session.incidents.length).toBe(3);
    }
  });
});
