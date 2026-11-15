import type { Session } from "../../types";
import { xFor, stepForIncident, type View } from "../../utils/layout";
import { incidentLabel } from "../../utils/format";

interface Props {
  session: Session;
  view: View;
  height: number;
  onSelectIncident: (incidentId: string) => void;
}

/**
 * Red triangles, one per incident, positioned at the *step* the incident
 * points at. The triangle carries the severity in its fill and the kind in
 * its label, so colour is never the only signal.
 */
export function IncidentTrack({ session, view, height, onSelectIncident }: Props) {
  return (
    <g className="track track-incidents">
      {session.incidents.map((incident, i) => {
        // `incident.step` indexes steps, not seq.
        const step = stepForIncident(session, incident.step);
        if (!step) return null;

        const x = xFor(step.t_ms, view);
        if (x < -240 || x > view.width + 240) return null;

        const critical = incident.severity === "critical";
        const y = height * 0.42;
        const size = 7;

        return (
          <g
            key={incident.id}
            className="mark-incident"
            role="button"
            tabIndex={0}
            aria-label={`Incident: ${incident.title} (${incidentLabel(incident.kind)}, ${incident.severity})`}
            style={{ cursor: "pointer" }}
            onClick={() => onSelectIncident(incident.id)}
            onKeyDown={(e) => {
              if (e.key === "Enter" || e.key === " ") onSelectIncident(incident.id);
            }}
          >
            <polygon
              points={`${x},${y - size} ${x - size},${y + size} ${x + size},${y + size}`}
              fill={critical ? "var(--red)" : "var(--orange)"}
              stroke="var(--paper)"
              strokeWidth={1}
            />
            <text
              x={x}
              y={y + 1.5}
              fontSize={8}
              fill="var(--paper)"
              fontFamily="var(--font-display)"
              fontWeight={700}
              textAnchor="middle"
            >
              !
            </text>
            <text
              x={x + size + 3}
              y={y + 3}
              fontSize={8}
              fill={critical ? "var(--red)" : "var(--orange)"}
              fontFamily="var(--font-display)"
              fontWeight={700}
              style={{ letterSpacing: "0.06em" }}
            >
              {`I${i + 1}`}
            </text>
          </g>
        );
      })}
    </g>
  );
}
