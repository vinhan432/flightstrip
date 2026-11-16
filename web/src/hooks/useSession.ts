// The composition hook: session + playback + route + selection in one place,
// so App.tsx is layout only and the deep-link resolution is testable.

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { Dispatch, SetStateAction } from "react";
import type { Incident, Session } from "../types";
import { useStore } from "../store";
import { usePlayback, type Playback } from "./usePlayback";
import { useHashRoute, type Route } from "./useHashRoute";
import { domain } from "../utils/layout";

export interface Focus {
  stepIndex: number;
  incident: Incident | null;
}

export interface ViewRect {
  t0: number;
  t1: number;
  width: number;
}

export interface SessionView {
  session: Session | null;
  playback: Playback;
  view: ViewRect;
  setView: Dispatch<SetStateAction<ViewRect>>;
  focus: Focus | null;
  openStep: (index: number, incident?: Incident | null) => void;
  closeFocus: () => void;
  route: Route;
  navigate: (route: Route) => void;
}

/** Loaded when the URL names no session. */
export const DEFAULT_SESSION_ID = "risky-session";

/**
 * Resolve a route's `focus` against a session. Accepts an incident id
 * (`i-0`) or a step index (`12`). An unknown focus resolves to null, which
 * means "load the session, open nothing" — not an error (Review Focus 6).
 */
export function resolveFocus(
  session: Session,
  focus: string | undefined,
): Focus | null {
  if (!focus) return null;

  const incident = session.incidents.find((i) => i.id === focus);
  if (incident) {
    return { stepIndex: incident.step, incident };
  }

  if (/^\d+$/.test(focus)) {
    const index = Number(focus);
    if (index >= 0 && index < session.steps.length) {
      return { stepIndex: index, incident: null };
    }
  }

  return null;
}

export function useSession(): SessionView {
  const { session, loadSession } = useStore();
  const [route, navigate] = useHashRoute();

  const duration = session ? domain(session).end : 0;
  const playback = usePlayback(session, duration);

  const [view, setView] = useState<ViewRect>({ t0: 0, t1: 1, width: 1200 });
  const [focus, setFocus] = useState<Focus | null>(null);
  const width = view.width;

  // The route names a session; load it. Without this, a deep link such as
  // `#/session/clean` would show whatever session happened to be loaded
  // while the URL and the picker both claimed otherwise.
  //
  // With no hash at all, fall back to the default so the page is never empty.
  const { manifest } = useStore();
  useEffect(() => {
    if (route.sessionId) {
      if (session?.meta.id === route.sessionId) return;
      void loadSession(route.sessionId);
      return;
    }
    if (!session && manifest.length > 0) {
      const fallback = manifest.find((m) => m.id === DEFAULT_SESSION_ID) ?? manifest[0];
      if (fallback) {
        navigate({ sessionId: fallback.id });
      }
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [route.sessionId, session, manifest, loadSession]);

  // Reset the view when the session changes.
  useEffect(() => {
    if (!session) return;
    setView({ t0: 0, t1: Math.max(1, duration), width });
    setFocus(null);
    // Deliberately keyed on the session, not the width: a resize must not
    // discard the user's zoom or close the drawer.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [session, duration]);

  // Apply the route's focus once the session is loaded.
  const appliedFocus = useRef<string | null>(null);
  useEffect(() => {
    if (!session || !route.focus) return;
    if (appliedFocus.current === route.focus) return;
    appliedFocus.current = route.focus;

    const resolved = resolveFocus(session, route.focus);
    if (resolved) {
      setFocus(resolved);
      playback.seek(session.steps[resolved.stepIndex]?.t_ms ?? 0);
    }
    // An unresolved focus is ignored on purpose.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [session, route.focus]);

  const openStep = useCallback(
    (index: number, incident: Incident | null = null) => {
      setFocus({ stepIndex: index, incident });
      if (session) playback.seek(session.steps[index]?.t_ms ?? 0);
    },
    [session, playback],
  );

  const closeFocus = useCallback(() => setFocus(null), []);

  return useMemo(
    () => ({
      session,
      playback,
      view,
      setView,
      focus,
      openStep,
      closeFocus,
      route,
      navigate,
    }),
    [session, playback, view, focus, openStep, closeFocus, route, navigate],
  );
}
