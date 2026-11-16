// The playback clock. This hook owns the only requestAnimationFrame loop in
// the app.
//
// The clock value is deliberately NOT React state. It ticks ~60 times a
// second; as state it re-rendered the entire page tree on every frame, which
// is what made the page stutter. Instead the value lives in a ref and is
// pushed to subscribers, so only the handful of leaf components that actually
// draw the clock re-render.
//
// `playing` and `speed` DO stay state: they change on a click, not per frame.

import { useCallback, useEffect, useRef, useState } from "react";
import type { Session } from "../types";

/** Pure clamp: advance `current` by `dt * speed`, bounded to [0, duration]. */
export function advance(
  current: number,
  dt: number,
  speed: number,
  duration: number,
): number {
  if (duration <= 0) return 0;
  if (dt <= 0) return Math.max(0, Math.min(current, duration));
  const next = current + dt * speed;
  return Math.max(0, Math.min(next, duration));
}

function relativeTimes(session: Session): number[] {
  return session.steps
    .filter((step) => step.seq !== 0 && step.kind !== "session_start")
    .map((step) => step.t_ms)
    .sort((a, b) => a - b);
}

/** The next step's time after `from`, or `from` at the end. */
export function nextStepTime(session: Session, from: number): number {
  const next = relativeTimes(session).find((t) => t > from + 1);
  return next ?? from;
}

/** The previous step's time before `from`, or `from` at the start. */
export function prevStepTime(session: Session, from: number): number {
  const times = relativeTimes(session).filter((t) => t < from - 1);
  return times.length > 0 ? times[times.length - 1] : Math.max(0, 0);
}

export interface Playback {
  playing: boolean;
  speed: number;
  play: () => void;
  pause: () => void;
  toggle: () => void;
  seek: (ms: number) => void;
  step: (delta: number) => void;
  setSpeed: (speed: number) => void;
  reset: () => void;
  /** Read the clock without subscribing. */
  getClock: () => number;
  /** Subscribe to clock changes; fires immediately, returns an unsubscribe. */
  subscribe: (listener: (ms: number) => void) => () => void;
}

export function usePlayback(
  session: Session | null,
  durationMs: number,
): Playback {
  const [playing, setPlaying] = useState(false);
  const [speed, setSpeed] = useState(1);

  const clock = useRef(0);
  const listeners = useRef(new Set<(ms: number) => void>());
  const frame = useRef(0);
  const last = useRef(0);
  const speedRef = useRef(speed);
  const durationRef = useRef(durationMs);

  speedRef.current = speed;
  durationRef.current = durationMs;

  /** Write the clock and push it to every subscriber. */
  const emit = useCallback((ms: number) => {
    clock.current = ms;
    for (const listener of listeners.current) listener(ms);
  }, []);

  const subscribe = useCallback((listener: (ms: number) => void) => {
    listeners.current.add(listener);
    // Hand the newcomer the current value so it paints correctly on mount.
    listener(clock.current);
    return () => {
      listeners.current.delete(listener);
    };
  }, []);

  const getClock = useCallback(() => clock.current, []);

  // A new session rewinds the clock.
  useEffect(() => {
    emit(0);
    setPlaying(false);
  }, [session, emit]);

  useEffect(() => {
    if (!playing) return;

    last.current = performance.now();

    const tick = (now: number) => {
      const dt = now - last.current;
      last.current = now;

      const duration = durationRef.current;
      const next = Math.min(
        advance(clock.current, dt, speedRef.current, duration),
        duration,
      );

      emit(next);

      if (next >= duration) {
        setPlaying(false);
        return;
      }
      frame.current = requestAnimationFrame(tick);
    };

    frame.current = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame.current);
  }, [playing, durationMs, emit]);

  const play = useCallback(() => {
    // Replay from the top when the clock is already at the end.
    if (clock.current >= durationRef.current) emit(0);
    setPlaying(true);
  }, [emit]);

  const pause = useCallback(() => setPlaying(false), []);
  const toggle = useCallback(
    () => (playing ? setPlaying(false) : play()),
    [playing, play],
  );

  const seek = useCallback(
    (ms: number) => {
      emit(Math.max(0, Math.min(ms, durationRef.current)));
    },
    [emit],
  );

  const step = useCallback(
    (delta: number) => {
      if (!session) return;
      const target =
        delta > 0
          ? nextStepTime(session, clock.current)
          : prevStepTime(session, clock.current);
      seek(target);
    },
    [session, seek],
  );

  const reset = useCallback(() => {
    seek(0);
    setPlaying(false);
  }, [seek]);

  // Stable identity across clock ticks: it depends only on the things that
  // change on a click. This is what stops the page re-rendering per frame.
  return {
    playing,
    speed,
    play,
    pause,
    toggle,
    seek,
    step,
    setSpeed,
    reset,
    getClock,
    subscribe,
  };
}
