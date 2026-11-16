// A controllable Playback for tests. The real hook owns a rAF loop; a test
// wants to set the clock and assert, so this exposes the same surface with a
// manual `setClock`.

import type { Playback } from "../hooks/usePlayback";

export interface FakePlayback extends Playback {
  /** Move the clock and notify every subscriber. */
  setClock: (ms: number) => void;
  /** How many times a subscriber was notified. */
  notifications: () => number;
}

export function makePlayback(initial = 0): FakePlayback {
  let clock = initial;
  let count = 0;
  const listeners = new Set<(ms: number) => void>();

  const emit = (ms: number) => {
    clock = ms;
    count++;
    for (const listener of listeners) listener(ms);
  };

  return {
    playing: false,
    speed: 1,
    play: () => {},
    pause: () => {},
    toggle: () => {},
    seek: (ms) => emit(ms),
    step: () => {},
    setSpeed: () => {},
    reset: () => emit(0),
    getClock: () => clock,
    subscribe: (listener) => {
      listeners.add(listener);
      listener(clock);
      return () => {
        listeners.delete(listener);
      };
    },
    setClock: (ms) => emit(ms),
    notifications: () => count,
  };
}
