// A formatted clock readout that re-renders on its own, without pulling the
// page through React on every animation frame.

import { useClock } from "../hooks/useClock";
import type { Playback } from "../hooks/usePlayback";
import { formatClock } from "../utils/format";

interface Props {
  playback: Playback;
}

export function ClockText({ playback }: Props) {
  const clockMs = useClock(playback);
  return <>{formatClock(clockMs)}</>;
}
