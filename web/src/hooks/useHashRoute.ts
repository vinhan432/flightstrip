// Hash routing. No server rewrites are needed, so a deep link works from any
// static host.
//
// The format is `#/s/<id>?step=N` for a session (the step is optional) and
// `#/report/<id>` for the investigation. Parsing is pure so the "unknown
// step" case can be tested without a browser.

import { useCallback, useEffect, useState } from "react";

export interface Route {
  kind: "session" | "report";
  sessionId: string;
  /** The selected step index, when the URL names one. */
  step?: number;
}

/** No hash at all: the caller falls back to a default session. */
export const EMPTY_ROUTE: Route = { kind: "session", sessionId: "" };

export function parseRoute(hash: string): Route {
  if (!hash) return EMPTY_ROUTE;
  const clean = hash.startsWith("#") ? hash.slice(1) : hash;

  const [path, query] = clean.split("?");
  const parts = path.split("/").filter(Boolean);

  if (parts[0] === "s" && parts[1]) {
    const route: Route = { kind: "session", sessionId: parts[1] };
    const step = new URLSearchParams(query ?? "").get("step");
    // A non-numeric or negative step is ignored, not an error.
    if (step !== null && /^\d+$/.test(step)) route.step = Number(step);
    return route;
  }

  if (parts[0] === "report" && parts[1]) {
    return { kind: "report", sessionId: parts[1] };
  }

  return EMPTY_ROUTE;
}

export function routeToHash(route: Route): string {
  if (!route.sessionId) return "#";
  if (route.kind === "report") return `#/report/${route.sessionId}`;

  const base = `#/s/${route.sessionId}`;
  return route.step !== undefined ? `${base}?step=${route.step}` : base;
}

export function useHashRoute(): [Route, (route: Route) => void] {
  const [route, setRoute] = useState<Route>(() =>
    parseRoute(window.location.hash),
  );

  useEffect(() => {
    const onChange = () => setRoute(parseRoute(window.location.hash));
    window.addEventListener("hashchange", onChange);
    return () => window.removeEventListener("hashchange", onChange);
  }, []);

  const navigate = useCallback((next: Route) => {
    const hash = routeToHash(next);
    if (window.location.hash !== hash) {
      window.location.hash = hash;
    }
    setRoute(next);
  }, []);

  return [route, navigate];
}
