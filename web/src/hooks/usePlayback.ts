// The playback clock. This hook owns the only requestAnimationFrame loop in
// the app and the only writer of `clockMs`; everything else derives from it.

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
  clockMs: number;
  playing: boolean;
  speed: number;
  play: () => void;
  pause: () => void;
  toggle: () => void;
  seek: (ms: number) => void;
  step: (delta: number) => void;
  setSpeed: (speed: number) => void;
  reset: () => void;
}

export function usePlayback(
  session: Session | null,
  durationMs: number,
): Playback {
  const [clockMs, setClockMs] = useState(0);
  const [playing, setPlaying] = useState(false);
  const [speed, setSpeed] = useState(1);

  const frame = useRef(0);
  const last = useRef(0);
  const clock = useRef(0);
  const speedRef = useRef(speed);

  speedRef.current = speed;

  // A new session rewinds the clock.
  useEffect(() => {
    clock.current = 0;
    setClockMs(0);
    setPlaying(false);
  }, [session]);

  useEffect(() => {
    if (!playing) return;

    last.current = performance.now();

    const tick = (now: number) => {
      const dt = now - last.current;
      last.current = now;

      const next = advance(clock.current, dt, speedRef.current, durationMs);
      const capped = Math.min(next, durationMs);

      clock.current = capped;
      setClockMs(capped);

      if (capped >= durationMs) {
        setPlaying(false);
        return;
      }
      frame.current = requestAnimationFrame(tick);
    };

    frame.current = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame.current);
  }, [playing, durationMs]);

  const play = useCallback(() => {
    // Replay from the top when the clock is already at the end.
    if (clock.current >= durationMs) {
      clock.current = 0;
      setClockMs(0);
    }
    setPlaying(true);
  }, [durationMs]);

  const pause = useCallback(() => setPlaying(false), []);
  const toggle = useCallback(
    () => (playing ? setPlaying(false) : play()),
    [playing, play],
  );

  const seek = useCallback(
    (ms: number) => {
      const clamped = Math.max(0, Math.min(ms, durationMs));
      clock.current = clamped;
      setClockMs(clamped);
    },
    [durationMs],
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

  return {
    clockMs,
    playing,
    speed,
    play,
    pause,
    toggle,
    seek,
    step,
    setSpeed,
    reset,
  };
}
