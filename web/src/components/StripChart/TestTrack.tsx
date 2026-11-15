import type { Session } from "../../types";
import { asTestRun } from "../../types";
import { xFor, type View } from "../../utils/layout";

interface Props {
  session: Session;
  view: View;
  height: number;
  onSelectStep: (index: number) => void;
}

/**
 * A step function of suite state: green while passing, red while failing.
 * Each run holds its state until the next one, which is what makes a
 * regression visible as a red block rather than a point.
 */
export function TestTrack({ session, view, height, onSelectStep }: Props) {
  const runs = session.steps
    .map((step, index) => ({ step, index, run: asTestRun(step) }))
    .filter((r) => r.run !== undefined);

  if (runs.length === 0) {
    return (
      <g className="track track-tests">
        <rect x={0} y={0} width={view.width} height={height} fill="var(--green-muted)" opacity={0.25} />
      </g>
    );
  }

  const blocks = runs.map((run, i) => {
    const from = xFor(run.step.t_ms, view);
    const next = runs[i + 1];
    const to = next ? xFor(next.step.t_ms, view) : view.width;
    return { ...run, from, to };
  });

  return (
    <g className="track track-tests">
      <rect x={0} y={0} width={view.width} height={height} fill="var(--green-muted)" opacity={0.25} />
      {blocks.map((block) => {
        const ok = block.run!.ok;
        const width = Math.max(2, block.to - block.from);
        if (block.from > view.width + 200 || block.to < -200) return null;
        return (
          <g
            key={block.index}
            role="button"
            tabIndex={0}
            aria-label={`Test run at step ${block.index}: ${block.run!.passed} passed, ${block.run!.failed} failed`}
            style={{ cursor: "pointer" }}
            onClick={() => onSelectStep(block.index)}
            onKeyDown={(e) => {
              if (e.key === "Enter" || e.key === " ") onSelectStep(block.index);
            }}
          >
            <rect
              x={block.from}
              y={height * 0.25}
              width={width}
              height={height * 0.5}
              fill={ok ? "var(--green)" : "var(--red)"}
              opacity={ok ? 0.55 : 0.85}
            />
            {!ok && (
              <text
                x={block.from + 4}
                y={height * 0.62}
                fontSize={9}
                fill="var(--paper)"
                fontFamily="var(--font-mono)"
              >
                {block.run!.failed} failed
              </text>
            )}
          </g>
        );
      })}
    </g>
  );
}
