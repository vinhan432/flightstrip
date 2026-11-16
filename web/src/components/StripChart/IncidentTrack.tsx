import { useEffect, useRef, useState } from "react";
import type { Session } from "../../types";
import { xFor, stepForIncident, type View } from "../../utils/layout";
import { incidentLabel } from "../../utils/format";
import type { Playback } from "../../hooks/usePlayback";

interface Props {
  session: Session;
  view: View;
  height: number;
  playback: Playback;
  onSelectIncident: (incidentId: string) => void;
}

/**
 * Red triangles, one per incident, positioned at the *step* the incident
 * points at. The triangle carries the severity in its fill and the kind in
 * its label, so colour is never the only signal.
 *
 * Each triangle stamps in as the pen reaches it during playback. It is never
 * hidden: the recording is a finished artifact, so the incident track must
 * read as populated even before the replay gets there.
 *
 * The clock is watched through a subscription that only updates when the SET
 * of reached incidents changes — a handful of times per replay, not 60 times
 * a second.
 */
export function IncidentTrack({
  session,
  view,
  height,
  playback,
  onSelectIncident,
}: Props) {
  const [reached, setReached] = useState<Set<string>>(() => new Set());
  const signature = useRef("");

  useEffect(() => {
    return playback.subscribe((ms) => {
      const now = new Set<string>();
      for (const incident of session.incidents) {
        const step = stepForIncident(session, incident.step);
        if (step && ms >= step.t_ms) now.add(incident.id);
      }

      const next = [...now].sort().join("|");
      if (next === signature.current) return;
      signature.current = next;
      setReached(now);
    });
  }, [playback, session]);

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
        const stamped = reached.has(incident.id);

        return (
          <g
            key={incident.id}
            className={`mark-incident${stamped ? " is-stamped" : ""}`}
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
