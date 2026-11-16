import { describe, it, expect } from "vitest";
import { parseRoute, routeToHash } from "./useHashRoute";

describe("parseRoute", () => {
  it("parses a session route", () =>
    expect(parseRoute("#/s/risky-session")).toEqual({
      kind: "session",
      sessionId: "risky-session",
    }));

  it("parses a step", () =>
    expect(parseRoute("#/s/risky-session?step=12")).toEqual({
      kind: "session",
      sessionId: "risky-session",
      step: 12,
    }));

  it("ignores a non-numeric step", () =>
    expect(parseRoute("#/s/risky-session?step=abc")).toEqual({
      kind: "session",
      sessionId: "risky-session",
    }));

  it("parses a report route", () =>
    expect(parseRoute("#/report/clean")).toEqual({
      kind: "report",
      sessionId: "clean",
    }));

  it("treats an empty hash as no session", () =>
    expect(parseRoute("")).toEqual({ kind: "session", sessionId: "" }));

  it("ignores a bare hash", () =>
    expect(parseRoute("#")).toEqual({ kind: "session", sessionId: "" }));

  it("ignores an unknown route", () =>
    expect(parseRoute("#/other/thing")).toEqual({
      kind: "session",
      sessionId: "",
    }));
});

describe("routeToHash", () => {
  it("round-trips a session", () =>
    expect(routeToHash({ kind: "session", sessionId: "clean" })).toBe(
      "#/s/clean",
    ));

  it("round-trips a step", () =>
    expect(routeToHash({ kind: "session", sessionId: "clean", step: 3 })).toBe(
      "#/s/clean?step=3",
    ));

  it("round-trips a report", () =>
    expect(routeToHash({ kind: "report", sessionId: "clean" })).toBe(
      "#/report/clean",
    ));

  it("drops an empty session", () =>
    expect(routeToHash({ kind: "session", sessionId: "" })).toBe("#"));
});
