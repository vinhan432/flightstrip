import { describe, it, expect } from "vitest";
import { graphLayout } from "./TraceGraph";
import type { Session } from "../types";
import weakened from "../../../fixtures/weakened-tests.json";
import risky from "../../../fixtures/risky-session.json";

const session = weakened as unknown as Session;

describe("graphLayout", () => {
  it("is deterministic", () => {
    expect(graphLayout(session)).toEqual(graphLayout(session));
  });

  it("sizes a file node by churn within 20-50px", () => {
    const g = graphLayout(session);
    expect(g.files.length).toBe(4);
    expect(g.files.every((f) => f.r! >= 20 && f.r! <= 50)).toBe(true);
  });

  it("places prompts in a left column and files to their right", () => {
    const g = graphLayout(session);
    const promptX = g.prompts[0].x;
    const fileX = g.files[0].x;
    expect(fileX).toBeGreaterThan(promptX);
  });

  it("gives every node a finite position", () => {
    const g = graphLayout(session);
    for (const node of [...g.prompts, ...g.files, ...g.incidents]) {
      expect(Number.isFinite(node.x)).toBe(true);
      expect(Number.isFinite(node.y)).toBe(true);
    }
  });

  it("returns empty columns for an empty session (Review Focus 5)", () => {
    const g = graphLayout({
      prompts: [],
      files: {},
      incidents: [],
      steps: [],
    } as unknown as Session);
    expect(g.prompts.length + g.files.length + g.incidents.length).toBe(0);
    expect(g.edges).toEqual([]);
  });

  it("connects prompts to the files they touched", () => {
    const g = graphLayout(session);
    expect(g.edges.length).toBeGreaterThan(0);
  });

  it("handles the risky session without a NaN", () => {
    const g = graphLayout(risky as unknown as Session);
    expect(g.files.every((f) => Number.isFinite(f.r!))).toBe(true);
  });
});
