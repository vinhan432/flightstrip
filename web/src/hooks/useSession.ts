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
  openStep: (index: number) => void;
  closeFocus: () => void;
  route: Route;
  navigate: (route: Route) => void;
}

/** Loaded when the URL names no session. */
export const DEFAULT_SESSION_ID = "risky-session";

/**
 * Resolve a step index against a session. The incident, when there is one, is
 * whichever points at that step. An out-of-range step resolves to null, which
 * means "load the session, open nothing" — not an error (Review Focus 6).
 */
export function resolveFocus(session: Session, stepIndex: number): Focus | null {
  if (stepIndex < 0 || stepIndex >= session.steps.length) return null;
  const incident = session.incidents.find((i) => i.step === stepIndex) ?? null;
  return { stepIndex, incident };
}

export function useSession(): SessionView {
  const { session, loadSession, manifest, loadedId } = useStore();
  const [route, navigate] = useHashRoute();

  const duration = session ? domain(session).end : 0;
  const playback = usePlayback(session, duration);

  const [view, setView] = useState<ViewRect>({ t0: 0, t1: 1, width: 1200 });
  const width = view.width;

  // The id currently being fetched. A ref, not state: it must be readable
  // synchronously inside the effect, before the next render.
  const requesting = useRef<string | null>(null);

  // The route names a session; load it. Without this, a deep link such as
  // `#/s/clean` would show whatever session happened to be loaded while the
  // URL and the picker both claimed otherwise.
  //
  // With no hash at all, fall back to the default so the page is never empty.
  //
  // The comparison is against `loadedId` (the catalogue key), NOT
  // `session.meta.id` (the recorder's own id). Those are different values, and
  // conflating them made this effect refetch on every render — an infinite
  // loop that left the chart on its skeleton forever.
  //
  // `status` is in the dependency list so a failed load is not retried in a
  // tight loop either.
  useEffect(() => {
    if (route.sessionId) {
      if (loadedId === route.sessionId) return;
      // Already fetching this one; do not start a second request.
      if (requesting.current === route.sessionId) return;
      requesting.current = route.sessionId;
      void loadSession(route.sessionId);
      return;
    }
    if (!session && manifest.length > 0) {
      const fallback =
        manifest.find((m) => m.id === DEFAULT_SESSION_ID) ?? manifest[0];
      if (fallback) navigate({ kind: "session", sessionId: fallback.id });
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [route.sessionId, loadedId, session, manifest, loadSession]);

  // Reset the view when the session changes. Deliberately keyed on the
  // session, not the width: a resize must not discard the user's zoom.
  useEffect(() => {
    if (!session) return;
    setView({ t0: 0, t1: Math.max(1, duration), width });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [session, duration]);

  // The selection IS the route's step. Deriving it rather than storing it is
  // what makes back/forward work with no extra bookkeeping.
  const focus = useMemo(
    () =>
      session && route.step !== undefined
        ? resolveFocus(session, route.step)
        : null,
    [session, route.step],
  );

  // Seek the clock to the selected step, once per distinct selection.
  const applied = useRef<string | null>(null);
  const seek = playback.seek;
  useEffect(() => {
    if (!session || route.step === undefined) {
      applied.current = null;
      return;
    }
    const key = `${session.meta.id}:${route.step}`;
    if (applied.current === key) return;
    applied.current = key;

    const step = session.steps[route.step];
    if (step) seek(step.t_ms);
  }, [session, route.step, seek]);

  const openStep = useCallback(
    (index: number) => {
      navigate({ kind: "session", sessionId: route.sessionId, step: index });
    },
    [navigate, route.sessionId],
  );

  const closeFocus = useCallback(() => {
    navigate({ kind: "session", sessionId: route.sessionId });
  }, [navigate, route.sessionId]);

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
