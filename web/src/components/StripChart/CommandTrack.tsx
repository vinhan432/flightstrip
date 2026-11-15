import type { Session } from "../../types";
import { asCommand } from "../../types";
import { xFor, type View } from "../../utils/layout";

interface Props {
  session: Session;
  view: View;
  height: number;
  onSelectStep: (index: number) => void;
}

/** Small ticks, one per command. A non-zero exit gets a red cap. */
export function CommandTrack({ session, view, height, onSelectStep }: Props) {
  const commands = session.steps
    .map((step, index) => ({ step, index, command: asCommand(step) }))
    .filter((c) => c.command !== undefined);

  return (
    <g className="track track-commands">
      <line
        x1={0}
        y1={height / 2}
        x2={view.width}
        y2={height / 2}
        stroke="var(--ink-faint)"
        strokeWidth={1}
        strokeDasharray="2 3"
      />
      {commands.map(({ step, index, command }) => {
        const x = xFor(step.t_ms, view);
        if (x < -20 || x > view.width + 20) return null;
        const failed = (command!.exit_code ?? 0) !== 0;
        return (
          <g
            key={index}
            role="button"
            tabIndex={0}
            aria-label={`Command: ${command!.cmd}${failed ? ` (exit ${command!.exit_code})` : ""}`}
            style={{ cursor: "pointer" }}
            onClick={() => onSelectStep(index)}
            onKeyDown={(e) => {
              if (e.key === "Enter" || e.key === " ") onSelectStep(index);
            }}
          >
            <line
              x1={x}
              y1={height * 0.2}
              x2={x}
              y2={height * 0.8}
              stroke={failed ? "var(--red)" : "var(--ink)"}
              strokeWidth={2}
            />
          </g>
        );
      })}
    </g>
  );
}
