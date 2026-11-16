// Subscribe a leaf component to the playback clock.
//
// The clock ticks ~60 times a second. Only components that actually draw the
// clock should re-render at that rate, so this hook keeps the value in local
// state and updates it from the playback subscription. Everything else in the
// tree stays untouched.
//
// The update is a plain setState: the clock emits at most once per animation
// frame, and React batches state updates within a frame, so this costs one
// render per frame with no extra scheduling of its own. (An earlier version
// coalesced through requestAnimationFrame; that added a frame of latency, so
// the pen visibly trailed the clock.)

import { useEffect, useState } from "react";
import type { Playback } from "./usePlayback";

export function useClock(playback: Playback): number {
  const [clockMs, setClockMs] = useState(() => playback.getClock());

  useEffect(() => playback.subscribe(setClockMs), [playback]);

  return clockMs;
}
