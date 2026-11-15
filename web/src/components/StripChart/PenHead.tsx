import { xFor, type View } from "../../utils/layout";

interface Props {
  view: View;
  clockMs: number;
  height: number;
}

/** The nib: an orange triangle that rides the chart at the current time. */
export function PenHead({ view, clockMs, height }: Props) {
  const x = xFor(clockMs, view);

  return (
    <g className="pen" aria-hidden="true">
      {/* The trail: a faint wash behind the nib that fades out. */}
      <rect
        x={Math.max(0, x - 26)}
        y={0}
        width={26}
        height={height}
        fill="url(#pen-trail-gradient)"
      />
      <line x1={x} y1={0} x2={x} y2={height} stroke="var(--orange)" strokeWidth={1} opacity={0.5} />
      <polygon
        points={`${x},${height / 2 - 6} ${x - 5},${height / 2 + 4} ${x + 5},${height / 2 + 4}`}
        fill="var(--orange)"
        stroke="var(--paper)"
        strokeWidth={1}
      />
    </g>
  );
}
