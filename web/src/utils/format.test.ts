import { describe, it, expect } from "vitest";
import { formatClock, formatDuration, promptText, fileRows } from "./format";
import type { Session } from "../types";
import weakened from "../../../fixtures/weakened-tests.json";

const session = weakened as unknown as Session;

describe("formatClock", () => {
  it("floors to seconds", () => expect(formatClock(83000)).toBe("1:23"));
  it("pads the seconds", () => expect(formatClock(5000)).toBe("0:05"));
  it("handles zero", () => expect(formatClock(0)).toBe("0:00"));
  it("rolls past an hour", () => expect(formatClock(3_723_000)).toBe("1:02:03"));
  it("clamps a negative to zero", () => expect(formatClock(-10)).toBe("0:00"));
});

describe("formatDuration", () => {
  it("renders minutes and seconds", () =>
    expect(formatDuration(99000)).toBe("1m 39s"));
  it("renders seconds alone", () =>
    expect(formatDuration(8000)).toBe("8s"));
  it("renders hours", () =>
    expect(formatDuration(3_723_000)).toBe("1h 2m"));
});

describe("promptText", () => {
  it("joins a prompt id to its text", () => {
    expect(promptText(session, "p-5")).toBe(
      "the upload retry test is failing, make the build green",
    );
  });
  it("returns empty for an unknown id", () =>
    expect(promptText(session, "p-99")).toBe(""));
  it("returns empty for undefined", () =>
    expect(promptText(session, undefined)).toBe(""));
});

describe("fileRows", () => {
  it("returns one row per file, sorted by churn then path", () => {
    const rows = fileRows(session);
    expect(rows.length).toBe(4);
    expect(rows[0].path).toBe("src/queue.test.ts");
  });
  it("joins prompt ids to text", () => {
    const row = fileRows(session).find((r) => r.path === "src/upload.ts")!;
    expect(row.promptLabels[0]).toContain("wire the retry helper");
  });
  it("survives an empty files map", () => {
    expect(fileRows({ files: {} } as unknown as Session)).toEqual([]);
  });
});
