// The scrubber: a draggable bar with incident markers on the track.
// Keyboard-operable as a slider, so it is reachable without a pointer.

import { useCallback, useRef } from "react";
import type { Session } from "../../types";
import { domain, stepForIncident } from "../../utils/layout";

interface Props {
  session: Session;
  clockMs: number;
  onSeek: (ms: number) => void;
  onSelectIncident?: (incidentId: string) => void;
}

export function Scrubber({ session, clockMs, onSeek, onSelectIncident }: Props) {
  const trackRef = useRef<HTMLDivElement | null>(null);
  const dragging = useRef(false);

  const { start, end } = domain(session);
  const span = Math.max(1, end - start);
  const ratio = Math.max(0, Math.min(1, (clockMs - start) / span));

  const seekFromClientX = useCallback(
    (clientX: number) => {
      const rect = trackRef.current?.getBoundingClientRect();
      if (!rect || rect.width === 0) return;
      const r = Math.max(0, Math.min(1, (clientX - rect.left) / rect.width));
      onSeek(start + r * span);
    },
    [onSeek, start, span],
  );

  const onPointerDown = useCallback(
    (event: React.PointerEvent) => {
      dragging.current = true;
      (event.target as Element).setPointerCapture?.(event.pointerId);
      seekFromClientX(event.clientX);
    },
    [seekFromClientX],
  );

  const onPointerMove = useCallback(
    (event: React.PointerEvent) => {
      if (dragging.current) seekFromClientX(event.clientX);
    },
    [seekFromClientX],
  );

  const onPointerUp = useCallback(() => {
    dragging.current = false;
  }, []);

  const onKeyDown = useCallback(
    (event: React.KeyboardEvent) => {
      const big = event.shiftKey ? 10000 : 1000;
      if (event.key === "ArrowRight") {
        event.preventDefault();
        onSeek(Math.min(end, clockMs + big));
      } else if (event.key === "ArrowLeft") {
        event.preventDefault();
        onSeek(Math.max(start, clockMs - big));
      } else if (event.key === "Home") {
        event.preventDefault();
        onSeek(start);
      } else if (event.key === "End") {
        event.preventDefault();
        onSeek(end);
      }
    },
    [clockMs, end, start, onSeek],
  );

  return (
    <div
      className="scrubber"
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onPointerCancel={onPointerUp}
    >
      <div
        ref={trackRef}
        className="scrubber-track"
        role="slider"
        tabIndex={0}
        aria-label="Scrub through the session"
        aria-valuemin={0}
        aria-valuemax={Math.round(span)}
        aria-valuenow={Math.round(clockMs - start)}
        onKeyDown={onKeyDown}
      >
        <div className="scrubber-fill" style={{ width: `${ratio * 100}%` }} />

        {session.incidents.map((incident) => {
          const step = stepForIncident(session, incident.step);
          if (!step) return null;
          const r = Math.max(0, Math.min(1, (step.t_ms - start) / span));
          return (
            <button
              key={incident.id}
              type="button"
              className="scrubber-incident"
              style={{ left: `${r * 100}%` }}
              aria-label={`Incident: ${incident.title}`}
              title={incident.title}
              onClick={(event) => {
                event.stopPropagation();
                onSeek(step.t_ms);
                onSelectIncident?.(incident.id);
              }}
            />
          );
        })}

        <div className="scrubber-thumb" style={{ left: `${ratio * 100}%` }} />
      </div>
    </div>
  );
}
