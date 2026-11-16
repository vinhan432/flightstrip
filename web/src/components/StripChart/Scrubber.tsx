// The scrubber: a draggable bar with incident markers on the track.
// Keyboard-operable as a slider, so it is reachable without a pointer.
//
// The playing position is drawn by a child that subscribes to the clock, so
// dragging and playback do not re-render the whole chart section.

import { useCallback, useRef } from "react";
import type { Session } from "../../types";
import { domain, stepForIncident } from "../../utils/layout";
import { useClock } from "../../hooks/useClock";
import type { Playback } from "../../hooks/usePlayback";

interface Props {
  session: Session;
  playback: Playback;
  onSelectIncident?: (incidentId: string) => void;
}

export function Scrubber({ session, playback, onSelectIncident }: Props) {
  const trackRef = useRef<HTMLDivElement | null>(null);
  const dragging = useRef(false);

  const { start, end } = domain(session);
  const span = Math.max(1, end - start);

  const seekFromClientX = useCallback(
    (clientX: number) => {
      const rect = trackRef.current?.getBoundingClientRect();
      if (!rect || rect.width === 0) return;
      const r = Math.max(0, Math.min(1, (clientX - rect.left) / rect.width));
      playback.seek(start + r * span);
    },
    [playback, start, span],
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
        onKeyDown={(event) => {
          const big = event.shiftKey ? 10000 : 1000;
          const now = playback.getClock();
          if (event.key === "ArrowRight") {
            event.preventDefault();
            playback.seek(Math.min(end, now + big));
          } else if (event.key === "ArrowLeft") {
            event.preventDefault();
            playback.seek(Math.max(start, now - big));
          } else if (event.key === "Home") {
            event.preventDefault();
            playback.seek(start);
          } else if (event.key === "End") {
            event.preventDefault();
            playback.seek(end);
          }
        }}
      >
        <ScrubberPosition
          playback={playback}
          start={start}
          span={span}
          session={session}
          onSelectIncident={onSelectIncident}
        />
      </div>
    </div>
  );
}

interface PositionProps {
  playback: Playback;
  start: number;
  span: number;
  session: Session;
  onSelectIncident?: (incidentId: string) => void;
}

/** The moving parts: fill, incident diamonds and thumb. */
function ScrubberPosition({
  playback,
  start,
  span,
  session,
  onSelectIncident,
}: PositionProps) {
  const clockMs = useClock(playback);
  const ratio = Math.max(0, Math.min(1, (clockMs - start) / span));

  return (
    <>
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
              playback.seek(step.t_ms);
              onSelectIncident?.(incident.id);
            }}
          />
        );
      })}

      <div className="scrubber-thumb" style={{ left: `${ratio * 100}%` }} />
      <span className="visually-hidden" aria-live="off">
        {Math.round(clockMs - start)} of {Math.round(span)} milliseconds
      </span>
    </>
  );
}
