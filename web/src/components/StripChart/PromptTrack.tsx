import type { Session } from "../../types";
import { xFor, type View } from "../../utils/layout";

interface Props {
  session: Session;
  view: View;
  height: number;
  onSelectStep: (index: number) => void;
}

/** Orange vertical lines, one per prompt, with a rotated label. */
export function PromptTrack({ session, view, height, onSelectStep }: Props) {
  return (
    <g className="track track-prompts">
      {session.prompts.map((prompt, i) => {
        const x = xFor(prompt.t_ms, view);
        if (x < -200 || x > view.width + 200) return null;
        return (
          <g
            key={prompt.id}
            className="mark-prompt"
            role="button"
            tabIndex={0}
            aria-label={`Prompt ${i + 1}: ${prompt.text}`}
            style={{ cursor: "pointer" }}
            onClick={() => onSelectStep(prompt.step)}
            onKeyDown={(e) => {
              if (e.key === "Enter" || e.key === " ") onSelectStep(prompt.step);
            }}
          >
            <line x1={x} y1={0} x2={x} y2={height} stroke="var(--orange)" strokeWidth={1.5} />
            <text
              x={x + 4}
              y={height - 6}
              fontSize={9}
              fill="var(--orange)"
              fontFamily="var(--font-display)"
              fontWeight={700}
              transform={`rotate(-90 ${x + 4} ${height - 6})`}
              style={{ letterSpacing: "0.08em" }}
            >
              P{i + 1}
            </text>
          </g>
        );
      })}
    </g>
  );
}
