import { xFor, type View } from "../../utils/layout";
import { useClock } from "../../hooks/useClock";
import type { Playback } from "../../hooks/usePlayback";

interface Props {
  view: View;
  playback: Playback;
  height: number;
}

/** How long the nib's trail lingers behind it, in session milliseconds. */
const TRAIL_MS = 2000;

/** The nib: an orange triangle that rides the chart at the current time. */
export function PenHead({ view, playback, height }: Props) {
  // Subscribes on its own: the nib moves every frame, and nothing else
  // should re-render because of it.
  const clockMs = useClock(playback);
  const x = xFor(clockMs, view);

  // The trail spans the last two seconds of travel, so it stretches when the
  // session is zoomed in and shrinks when it is zoomed out.
  const trailX = xFor(Math.max(view.t0, clockMs - TRAIL_MS), view);
  const trailW = Math.max(0, x - trailX);

  return (
    <g className="pen" aria-hidden="true">
      {trailW > 0 && (
        <rect
          x={trailX}
          y={0}
          width={trailW}
          height={height}
          fill="url(#pen-trail-gradient)"
        />
      )}
      <line
        x1={x}
        y1={0}
        x2={x}
        y2={height}
        stroke="var(--orange)"
        strokeWidth={1}
        opacity={0.5}
      />
      <polygon
        points={`${x},${height / 2 - 6} ${x - 5},${height / 2 + 4} ${x + 5},${height / 2 + 4}`}
        fill="var(--orange)"
        stroke="var(--paper)"
        strokeWidth={1}
      />
    </g>
  );
}
