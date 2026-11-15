import { describe, it, expect } from "vitest";
import { parseRoute, routeToHash } from "./useHashRoute";

describe("parseRoute", () => {
  it("parses a session route", () =>
    expect(parseRoute("#/session/risky-session")).toEqual({
      sessionId: "risky-session",
    }));

  it("parses a focus route", () =>
    expect(parseRoute("#/session/risky-session/i-0")).toEqual({
      sessionId: "risky-session",
      focus: "i-0",
    }));

  it("keeps an unknown focus for the caller to resolve (Review Focus 6)", () =>
    expect(parseRoute("#/session/risky-session/i-99")).toEqual({
      sessionId: "risky-session",
      focus: "i-99",
    }));

  it("treats an empty hash as no session", () =>
    expect(parseRoute("")).toEqual({ sessionId: "" }));

  it("ignores a bare hash", () =>
    expect(parseRoute("#")).toEqual({ sessionId: "" }));

  it("ignores a non-session hash", () =>
    expect(parseRoute("#/other/thing")).toEqual({ sessionId: "" }));
});

describe("routeToHash", () => {
  it("round-trips a session", () =>
    expect(routeToHash({ sessionId: "clean" })).toBe("#/session/clean"));

  it("round-trips a focus", () =>
    expect(routeToHash({ sessionId: "clean", focus: "i-1" })).toBe(
      "#/session/clean/i-1",
    ));

  it("drops an empty focus", () =>
    expect(routeToHash({ sessionId: "clean", focus: "" })).toBe(
      "#/session/clean",
    ));
});
