// Hash routing. No server rewrites are needed, so a deep link works from any
// static host.
//
// The route is `#/session/<id>` and optionally `#/session/<id>/<focus>`,
// where `<focus>` is an incident id (`i-0`) or a step index (`12`). Parsing
// is pure so the "unknown focus" case can be tested without a browser.

import { useCallback, useEffect, useState } from "react";

export interface Route {
  sessionId: string;
  focus?: string;
}

export function parseRoute(hash: string): Route {
  if (!hash) return { sessionId: "" };
  const clean = hash.startsWith("#") ? hash.slice(1) : hash;
  const parts = clean.split("/").filter(Boolean);

  // Expect: ["session", "<id>", "<focus>?"]
  if (parts[0] !== "session" || !parts[1]) return { sessionId: "" };

  const route: Route = { sessionId: parts[1] };
  if (parts[2]) route.focus = parts[2];
  return route;
}

export function routeToHash(route: Route): string {
  if (!route.sessionId) return "#";
  const base = `#/session/${route.sessionId}`;
  return route.focus ? `${base}/${route.focus}` : base;
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
