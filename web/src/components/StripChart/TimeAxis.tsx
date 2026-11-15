import type { Session } from "../../types";
import { domain, xFor, type View } from "../../utils/layout";
import { formatClock } from "../../utils/format";

interface Props {
  session: Session;
  view: View;
}

/** Top track: tick marks and timestamps across the session. */
export function TimeAxis({ session, view }: Props) {
  const { end } = domain(session);
  const height = 32;

  // Pick a tick step that yields roughly 6-12 labels at this zoom.
  const visibleSpan = view.t1 - view.t0;
  const rough = visibleSpan / 8;
  const steps = [1000, 2000, 5000, 10000, 15000, 30000, 60000, 120000, 300000];
  const stepMs = steps.find((s) => s >= rough) ?? 600000;

  const ticks: number[] = [];
  const first = Math.ceil(view.t0 / stepMs) * stepMs;
  for (let t = first; t <= Math.min(view.t1, end); t += stepMs) {
    ticks.push(t);
  }

  return (
    <g className="track track-axis">
      <line x1={0} y1={height} x2={view.width} y2={height} stroke="var(--ink-faint)" strokeWidth={1} />
      {ticks.map((t) => {
        const x = xFor(t, view);
        return (
          <g key={t}>
            <line x1={x} y1={height - 6} x2={x} y2={height} stroke="var(--ink-faint)" strokeWidth={1} />
            <text x={x + 3} y={height - 10} fontSize={9} fill="var(--ink-light)" fontFamily="var(--font-mono)">
              {formatClock(t)}
            </text>
          </g>
        );
      })}
    </g>
  );
}
